//! Port of `qbopt/frontend/raising_call_memory.py`.

use std::cmp::Reverse;
use std::collections::BTreeSet;
use std::rc::Rc;

use iced_x86::Register;
use num_bigint::BigInt;

use crate::abi::runtime::{self, Contract, Control, Memory};
use crate::analysis::{alias, consts, effects};
use crate::model::ir::decode::BodyIR;
use crate::model::ir::nodes::{Node, span};
use crate::model::ir::{Loc, Operation};
use crate::model::memory::{Identity, MemoryKind, MemoryObject, Provenance};
use crate::model::mir::{Arg, Kind, MemRef, MirBody, Op, RaisedBody, Value, WHOLE_FRAME};
use crate::objectfile::module::{self, Addr, Module, Space};
use crate::objectfile::{cvinfo, omf};
use crate::support::hash::IndexMap;

pub use crate::model::mir::Reach;

// Where a routine leaves control is a different question from what it can
// write, and the bound returned here answers only the second. INLINE_TABLE
// resumes at one of the table's own targets, which the raise already models
// as the dispatch block's successors -- so the data bound holds across it
// exactly as it holds across a return.
//
// Excluding it cost the `jumps` target: B$OGTA writes at the same Memory
// level as every B$P* output routine beside it in that loop, and those are
// bounded. Unbounded, its store aliased every cell in the program once per
// iteration, so nothing was promotable, so `induction.basics` saw no
// counter, so the trip count was unknown and neither expansion would run.
const _BOUNDED_CONTROL: [Control; 3] = [Control::Returns, Control::Never, Control::InlineTable];

/// Python's local `frame`: the frame offset a pointer argument's copies lead to.
fn frame(definitions: &IndexMap<Value, &Op>, arg: Option<&Arg>) -> Option<i64> {
    let mut seen = BTreeSet::new();
    let mut arg = arg?;
    while let Arg::Held(held) = arg {
        if !seen.insert(held.value) {
            break;
        }
        let op = definitions.get(&held.value)?;
        if !op.loads.is_empty() || !op.stores.is_empty() || op.barrier() || op.args.len() != 1 {
            return None;
        }
        if let (Kind::Address, Arg::FrameAddress(address)) = (op.kind, &op.args[0]) {
            return Some(address.offset);
        }
        if op.kind != Kind::Copy {
            return None;
        }
        arg = &op.args[0];
    }
    None
}

fn _definitions(body: &MirBody) -> IndexMap<Value, &Op> {
    body.blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| op.defines.iter().map(move |&value| (value, op)))
        .collect()
}

/// Bind a typed BASIC function's indirect result to its actual cell.
///
/// SINGLE and DOUBLE functions receive a final hidden near pointer where the
/// callee stores its answer.  The decoded call knew the result width but not
/// that pointer, so its anonymous write killed every caller-frame value.
/// CodeView establishes the non-INTEGER return class and the ordinary ARG
/// contract identifies the exact hidden argument.  Objects without both
/// facts retain the conservative call effect.
pub fn indirect_results(body: RaisedBody, found: &Module) -> RaisedBody {
    let parsed = cvinfo::parse(&found.records);
    let signatures: IndexMap<String, Option<String>> = parsed
        .procedures
        .iter()
        .filter_map(|procedure| {
            let signature = procedure.signature()?;
            Some((procedure.name.to_uppercase(), cvinfo::type_name(signature.return_type, Some(&*procedure.types))))
        })
        .collect();
    let definitions = _definitions(&body);
    let widths = |name: &str| match name {
        "SINGLE" => Some(4),
        "DOUBLE" => Some(8),
        _ => None,
    };
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut ops = block.ops.clone();
        for index in 0..ops.len() {
            let call = &ops[index];
            let width = match found.calls.get(&call.at).map(|name| signatures.get(&name.to_uppercase())) {
                Some(Some(Some(returned))) => widths(returned),
                _ => None,
            };
            let (Kind::Call, Some(width)) = (call.kind, width) else {
                continue;
            };
            let mut argument = None;
            for prior in ops[..index].iter().rev() {
                if prior.kind == Kind::Call {
                    break;
                }
                if prior.kind == Kind::Arg && prior.args.len() == 1 {
                    argument = Some(&prior.args[0]);
                    break;
                }
            }
            let destination = frame(&definitions, argument);
            let unknown: Vec<usize> = call
                .stores
                .iter()
                .enumerate()
                .filter(|(_, one)| one.space != Some(Space::Stack) && one.addr.is_none() && one.provenance.is_none())
                .map(|(position, _)| position)
                .collect();
            let Some(destination) = destination else {
                continue;
            };
            if unknown.len() != 1 || call.stores[unknown[0]].width != width {
                continue;
            }
            let mut made = call.clone();
            made.stores[unknown[0]] =
                MemRef { space: Some(Space::Frame), ..MemRef::new(Some(Addr::new(Space::Frame, destination)), width) };
            ops[index] = made;
        }
        blocks.push(block.with_ops(ops));
    }
    body.with_blocks(blocks)
}

/// Prove BASIC functions whose only write is their hidden result.
///
/// CodeView supplies the BYREF parameter slots that the machine frontend
/// could not otherwise distinguish from integers.  Those slots become the
/// same PARAMETER objects used by the C frontend's whole-module mod/ref
/// analysis.  A floating function is result-only only when the fixed-point
/// summary contains no unknown write and every written slice belongs to its
/// final hidden-result parameter.
pub fn result_only_functions(bodies: &[(&str, &MirBody)], found: &Module) -> Result<BTreeSet<String>, String> {
    let parsed = cvinfo::parse(&found.records);
    let procedures: IndexMap<i64, &cvinfo::Procedure> =
        parsed.procedures.iter().map(|procedure| (procedure.offset, procedure)).collect();
    let mut candidates: IndexMap<String, (alias::Procedure, i64)> = IndexMap::default();
    for (_label, body) in bodies {
        let Some(procedure) = procedures.get(&body.entry) else {
            continue;
        };
        let Some(signature) = procedure.signature() else {
            continue;
        };
        let returned = cvinfo::type_name(signature.return_type, Some(&*procedure.types));
        if !matches!(returned.as_deref(), Some("SINGLE" | "DOUBLE")) {
            continue;
        }
        let mut params = procedure.params();
        params.sort_by_key(|parameter| Reverse(parameter.bp_offset));
        let hidden = params.len() as i64;
        let mut offsets: IndexMap<i64, i64> =
            params.iter().enumerate().map(|(index, parameter)| (parameter.bp_offset, index as i64)).collect();
        offsets.insert(offsets.keys().min().copied().unwrap_or(8) - 2, hidden);
        let mut seeds = body.pointer_seeds.clone();
        for block in &body.blocks {
            for op in &block.ops {
                if op.kind != Kind::Load || op.results.len() != 1 || op.loads.len() != 1 {
                    continue;
                }
                let (Arg::Held(result), reference) = (&op.results[0], &op.loads[0]) else {
                    continue;
                };
                let Some(addr) = reference.addr else {
                    continue;
                };
                if addr.space != Space::Frame || reference.base.is_some() || reference.segment.is_some() {
                    continue;
                }
                if let Some(&index) = offsets.get(&addr.disp) {
                    seeds.insert(
                        result.value,
                        Provenance::one(MemoryObject {
                            identity: Some(Identity::Int(index)),
                            ..MemoryObject::new(MemoryKind::Parameter)
                        }),
                    );
                }
            }
        }
        let mut seeding = MirBody::clone(body);
        seeding.pointer_values.extend(seeds.keys().copied());
        seeding.pointer_seeds = seeds;
        let seeded = alias::annotated(&Rc::new(seeding))?;
        let calls: IndexMap<i64, String> = seeded
            .blocks
            .iter()
            .flat_map(|block| &block.ops)
            .filter(|op| op.kind == Kind::Call)
            .filter_map(|op| found.calls.get(&op.at).map(|name| (op.at, name.clone())))
            .collect();
        candidates.insert(
            procedure.name.to_uppercase(),
            (alias::Procedure { body: seeded, calls, arguments: IndexMap::default(), named: Default::default(), outside: Default::default() }, hidden),
        );
    }

    // The BASIC frame helpers implement this body's activation and carry no
    // source-language caller-memory effect on the ordinary edge.
    let empty = alias::Summary::default();
    let known: IndexMap<String, alias::Summary> =
        [("B$ENRA".to_owned(), empty.clone()), ("B$EXSA".to_owned(), empty)].into_iter().collect();
    let mut hiddens: IndexMap<String, i64> = IndexMap::default();
    let mut chosen: IndexMap<String, alias::Procedure> = IndexMap::default();
    for (name, (procedure, hidden)) in candidates {
        hiddens.insert(name.clone(), hidden);
        chosen.insert(name, procedure);
    }
    let summaries = alias::summaries(&chosen, Some(&known))?;
    Ok(hiddens
        .iter()
        .filter(|(name, hidden)| {
            let summary = &summaries[*name];
            !summary.unknown_write
                && !summary.writes.is_empty()
                && summary.writes.iter().all(|one| {
                    one.object.kind == MemoryKind::Parameter && one.object.identity == Some(Identity::Int(**hidden))
                })
        })
        .map(|(name, _)| name.clone())
        .collect())
}

/// Record the completed mod/ref proof on direct result-only calls.
pub fn complete_result_calls(
    mut body: RaisedBody,
    calls: &IndexMap<i64, String>,
    result_only: &BTreeSet<String>,
) -> RaisedBody {
    for block in &mut body.body_mut().blocks {
        for op in &mut block.ops {
            if op.kind == Kind::Call
                && result_only.contains(&calls.get(&op.at).map_or_else(String::new, |name| name.to_uppercase()))
            {
                op.memory_complete = true;
            }
        }
    }
    body
}

/// Give fixed-length B$ASSN calls their actual caller-memory ranges.
///
/// B$ASSN is the runtime spelling of both string assignment and a fixed UDT
/// copy.  With nonzero equal source/destination lengths it reads exactly the
/// source byte range and writes exactly the destination byte range; the six
/// words carrying those facts are explicit ARG operations.  Keep the call
/// itself as language-runtime scaffolding, but do not let its conservative
/// error paths alias unrelated frame objects.
///
/// Only frame addresses and FAR dynamic-array operands are admitted.  A
/// descriptor string, unequal padding/truncation, unknown segment, or
/// unresolved size retains the original conservative effect.
pub fn fixed_assignments(body: RaisedBody, found: &Module) -> RaisedBody {
    let facts = consts::known(&Rc::new(body.body.clone()), None, None, None, None);
    let definitions = _definitions(&body);
    let parsed = cvinfo::parse(&found.records);
    let procedure = parsed.procedures.iter().find(|procedure| procedure.offset == body.entry);
    let array_parameters: BTreeSet<i64> = match procedure {
        Some(procedure) => procedure
            .params()
            .into_iter()
            .filter(|parameter| parameter.type_name().unwrap_or_default().starts_with("BYREF ARRAY OF "))
            .map(|parameter| parameter.bp_offset)
            .collect(),
        None => BTreeSet::new(),
    };

    let number = |arg: &Arg| -> Option<BigInt> {
        match arg {
            Arg::Const(one) => Some(one.n.clone()),
            Arg::Held(held) => facts.get(&held.value).map(|fact| consts::masked(&fact.n, held.width)),
            _ => None,
        }
    };

    let array_descriptor = |value: Value| -> bool {
        let Some(op) = definitions.get(&value) else {
            return false;
        };
        if op.kind != Kind::Load || op.loads.len() != 1 {
            return false;
        }
        let reference = &op.loads[0];
        reference.addr.is_some_and(|addr| {
            addr.space == Space::Frame
                && reference.base.is_none()
                && reference.segment.is_none()
                && array_parameters.contains(&addr.disp)
        })
    };

    let dynamic_array_pointer = |arg: &Arg| -> bool {
        let Arg::Held(held) = arg else {
            return false;
        };
        let Some(op) = definitions.get(&held.value) else {
            return false;
        };
        if !matches!(op.kind, Kind::Add | Kind::Copy) {
            return false;
        }
        op.args.iter().any(|source| {
            let Arg::Cell(cell) = source else {
                return false;
            };
            cell.r#ref.addr.is_some_and(|addr| addr.disp == 10) && cell.r#ref.base.is_some_and(array_descriptor)
        })
    };

    let reference = |segment: &Arg, pointer: &Arg, width: u32| -> Option<MemRef> {
        let Arg::Opaque(segment) = segment else {
            return None;
        };
        let Some(Loc::Reg(register)) = segment.machine_payload() else {
            return None;
        };
        if register.register == Register::DS {
            if let Some(offset) = frame(&definitions, Some(pointer)) {
                return Some(MemRef {
                    space: Some(Space::Frame),
                    ..MemRef::new(Some(Addr::new(Space::Frame, offset)), width)
                });
            }
        }
        if register.register == Register::ES && matches!(pointer, Arg::Held(_)) {
            // The selector and offset came from a dynamic-array descriptor.
            // Its allocation predates the current activation and therefore
            // cannot be one of this activation's frame objects.  FAR alone is
            // not enough: an arbitrary far pointer may use SS.  State the
            // object-lifetime proof explicitly, while not retaining the
            // historical offset as a live call operand after ARG pushed it.
            let excludes = if dynamic_array_pointer(pointer) { vec![WHOLE_FRAME] } else { Vec::new() };
            return Some(MemRef {
                space: Some(Space::Far),
                base_width: 2,
                excludes,
                ..MemRef::new(None, width)
            });
        }
        None
    };

    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut ops = block.ops.clone();
        for index in 0..ops.len() {
            let call = &ops[index];
            if call.kind != Kind::Call || found.calls.get(&call.at).map(String::as_str) != Some("B$ASSN") {
                continue;
            }
            let mut arguments = Vec::new();
            for prior in ops[..index].iter().rev() {
                if prior.kind == Kind::Call {
                    break;
                }
                if prior.kind == Kind::Arg && prior.args.len() == 1 {
                    arguments.push(&prior.args[0]);
                    if arguments.len() == 6 {
                        break;
                    }
                }
            }
            arguments.reverse();
            let [source_segment, source_pointer, source_count, dest_segment, dest_pointer, dest_count] =
                arguments[..]
            else {
                continue;
            };
            let (source_width, dest_width) = (number(source_count), number(dest_count));
            let Some(source_width) = source_width else {
                continue;
            };
            if source_width <= BigInt::from(0) || Some(&source_width) != dest_width.as_ref() {
                continue;
            }
            let Ok(width) = u32::try_from(&source_width) else {
                continue;
            };
            let source = reference(source_segment, source_pointer, width);
            let destination = reference(dest_segment, dest_pointer, width);
            let (Some(source), Some(destination)) = (source, destination) else {
                continue;
            };
            let mut made = call.clone();
            made.stores = call.stores.iter().filter(|one| one.space == Some(Space::Stack)).cloned().collect();
            made.stores.push(destination);
            made.loads = vec![source];
            made.memory_complete = true;
            ops[index] = made;
        }
        blocks.push(block.with_ops(ops));
    }
    body.with_blocks(blocks)
}

pub fn reachable(
    contract: Option<&Contract>,
    access: Memory,
    escaped: Option<&Reach>,
    handles_errors: bool,
) -> Option<Reach> {
    let contract = contract?;
    let escaped = escaped?;
    if runtime::barrier(contract)
        || (contract.raises_error && handles_errors)
        || !_BOUNDED_CONTROL.contains(&contract.control)
        || access == Memory::Any
    {
        return None;
    }
    // NONE excludes caller data, not runtime scratch: B$FCMP writes DGROUP.
    Some(if access <= Memory::Arguments { (escaped.0, BTreeSet::new()) } else { escaped.clone() })
}

/// A callee's own footprint, independent of where control goes next.
///
/// `reachable` is the normal-return answer and therefore rejects callbacks
/// and error transfers.  An error-handler summary instead stops at the
/// transfer back to the interrupted body, so it needs the runtime routine's
/// direct footprint without conflating that later user code with the call.
fn _direct_reach(contract: Option<&Contract>, access: Memory, escaped: Option<&Reach>) -> Option<Reach> {
    let contract = contract?;
    let escaped = escaped?;
    if !contract.established || access == Memory::Any {
        return None;
    }
    Some(if access <= Memory::Arguments { (escaped.0, BTreeSet::new()) } else { escaped.clone() })
}

/// A handler's caller-memory reads and writes, as `handler_effects` returns them.
pub type HandlerSummary = (Vec<MemRef>, Vec<MemRef>);

/// Complete caller-memory effects before an error handler resumes.
///
/// The resumed program is represented by its own MIR and is not part of the
/// handler's footprint.  Unknown user calls still refuse the summary.  This
/// is a mod/ref summary, not an attempt to inline the handler's control flow.
pub fn handler_effects(
    body: &MirBody,
    calls: &IndexMap<i64, String>,
    contracts: &IndexMap<i64, Contract>,
    escaped: Option<&Reach>,
) -> Option<HandlerSummary> {
    let (mut reads, mut writes) = (Vec::new(), Vec::new());
    for block in &body.blocks {
        for op in &block.ops {
            if op.kind == Kind::Call && calls.contains_key(&op.at) {
                let contract = contracts.get(&op.at);
                let direct_reads = match contract {
                    Some(one) => one.direct_reads.unwrap_or(one.reads),
                    None => Memory::Any,
                };
                let direct_writes = match contract {
                    Some(one) => one.direct_writes.unwrap_or(one.writes),
                    None => Memory::Any,
                };
                let read_reach = _direct_reach(contract, direct_reads, escaped)?;
                let write_reach = _direct_reach(contract, direct_writes, escaped)?;
                let contract = contract.expect("a reach needs a contract");
                if direct_reads > Memory::Arguments {
                    reads.push(MemRef { beyond: Some(read_reach), ..MemRef::new(None, 0) });
                }
                if direct_writes > Memory::Arguments {
                    writes.push(MemRef { beyond: Some(write_reach), ..MemRef::new(None, 0) });
                }
                if contract.error_handling && contract.control == Control::Never {
                    break;
                }
                continue;
            }
            if effects::unmodeled_read(op) || effects::unmodeled_write(op) {
                return None;
            }
            reads.extend(op.loads.iter().filter(|one| one.space != Some(Space::Stack)).cloned());
            writes.extend(op.stores.iter().filter(|one| one.space != Some(Space::Stack)).cloned());
        }
    }
    Some((reads, writes))
}

/// Join a precise handler mod/ref summary into each error-capable call.
pub fn with_handler_effects(
    mut body: RaisedBody,
    summary: Option<&HandlerSummary>,
    calls: &IndexMap<i64, String>,
    contracts: &IndexMap<i64, Contract>,
    escaped: Option<&Reach>,
) -> RaisedBody {
    let Some((handler_reads, handler_writes)) = summary else {
        return body;
    };

    let bounded = |reference: &MemRef, reach: &Reach| -> MemRef {
        if reference.space == Some(Space::Stack) {
            reference.clone()
        } else {
            MemRef { beyond: Some(reach.clone()), ..reference.clone() }
        }
    };

    for block in &mut body.body_mut().blocks {
        for op in &mut block.ops {
            let contract =
                if op.kind == Kind::Call && calls.contains_key(&op.at) { contracts.get(&op.at) } else { None };
            let Some(contract) = contract else {
                continue;
            };
            if contract.raises_error && !contract.enters_user_code {
                let read_reach = reachable(Some(contract), contract.reads, escaped, false);
                let write_reach = reachable(Some(contract), contract.writes, escaped, false);
                if let (Some(read_reach), Some(write_reach)) = (read_reach, write_reach) {
                    op.loads = op
                        .loads
                        .iter()
                        .map(|one| bounded(one, &read_reach))
                        .chain(handler_reads.iter().cloned())
                        .collect();
                    op.stores = op
                        .stores
                        .iter()
                        .map(|one| bounded(one, &write_reach))
                        .chain(handler_writes.iter().cloned())
                        .collect();
                    op.memory_complete = true;
                }
            }
        }
    }
    body
}

// An exclusion covering every displacement of one extern.
const _WHOLE_SYMBOL: (i64, u32) = (-(1 << 15), 1 << 16);

/// Per call site, the runtime cells its callee is known not to write.
///
/// `runtime::WRITERS` says which routines write a cell. A call writes it when
/// it is to one of them, or to the program's own procedure that stores it or
/// calls one, or to anything that can run the program's code, or to what
/// nobody can name. A cell whose address this module hands out is written
/// by whoever holds the address, and is spared nowhere.
///
/// The answer is an exclusion on the callee's write, so every alias query
/// reads it -- memory SSA, availability and constant cells alike.
pub fn spared(
    found: &Module,
    decoded: &[BodyIR],
    contracts: &IndexMap<i64, Contract>,
) -> IndexMap<i64, Vec<(Addr, u32)>> {
    let family = module::family(&found.records);
    let names = omf::externals(&found.records);
    let mut cells: IndexMap<i64, &BTreeSet<&str>> = names
        .iter()
        .enumerate()
        .filter_map(|(index, name)| {
            runtime::WRITERS
                .iter()
                .find(|((writer, of), _)| *writer == name.as_str() && *of == family.value())
                .map(|(_, writers)| (index as i64, writers))
        })
        .collect();
    if cells.is_empty() {
        return IndexMap::default();
    }

    let nodes = decoded.iter().flat_map(|body| &body.nodes);
    for fixup in omf::fixups(&found.records) {
        if fixup.target == "external" && cells.contains_key(&fixup.index) && fixup.seg != Some(found.seg) {
            cells.shift_remove(&fixup.index);
        }
    }
    for node in nodes {
        let semantics = node.semantics();
        for one in semantics.dests.iter().chain(&semantics.sources) {
            let addr = match one {
                Loc::Imm(imm) => imm.address,
                Loc::Address(address) => address.addr,
                _ => None,
            };
            if let Some(addr) = addr.filter(|addr| addr.space == Space::External) {
                cells.shift_remove(&addr.index);
            }
        }
    }
    if cells.is_empty() {
        return IndexMap::default();
    }

    let defined = module::defines(&found.records, found.seg);
    let handles = runtime::handles_errors(contracts.values());
    let procedures: IndexMap<&str, &BodyIR> = decoded
        .iter()
        .filter_map(|body| body.body.name.as_deref().filter(|name| !name.is_empty()).map(|name| (name, body)))
        .collect();

    let calling = |at: i64, index: i64, writes: &IndexMap<&str, BTreeSet<i64>>| -> bool {
        let Some(name) = found.calls.get(&at) else {
            return true;
        };
        if defined.contains(name) {
            return !procedures.contains_key(name.as_str()) || writes[name.as_str()].contains(&index);
        }
        if cells[&index].contains(name.as_str()) {
            return true;
        }
        match contracts.get(&at) {
            None => true,
            Some(contract) => {
                contract.enters_user_code || contract.error_handling || (contract.raises_error && handles)
            }
        }
    };

    let storing = |node: &Node, index: i64| -> bool {
        let (semantics, effects) = (node.semantics(), node.effects());
        if semantics.op == Operation::Barrier && !effects.memory_complete {
            return true;
        }
        if semantics.op == Operation::Push {
            return false;
        }
        effects.stores.iter().any(|cell| match cell.addr {
            None => true,
            Some(addr) => {
                matches!(addr.space, Space::Far | Space::Literal | Space::Group)
                    || (addr.space == Space::External && addr.index == index)
            }
        })
    };

    let mut writes: IndexMap<&str, BTreeSet<i64>> = procedures.keys().map(|&name| (name, BTreeSet::new())).collect();
    let mut changing = true;
    while changing {
        changing = false;
        for (&name, body) in &procedures {
            for node in &body.nodes {
                let at = span(node).0 as i64;
                for &index in cells.keys() {
                    if writes[name].contains(&index) {
                        continue;
                    }
                    let call = node.semantics().op == Operation::Call;
                    if if call { calling(at, index, &writes) } else { storing(node, index) } {
                        writes[name].insert(index);
                        changing = true;
                    }
                }
            }
        }
    }

    let mut out = IndexMap::default();
    for &at in found.calls.keys() {
        let kept: Vec<(Addr, u32)> = cells
            .keys()
            .filter(|&&index| !calling(at, index, &writes))
            .map(|&index| (Addr { index, ..Addr::new(Space::External, _WHOLE_SYMBOL.0) }, _WHOLE_SYMBOL.1))
            .collect();
        if !kept.is_empty() {
            out.insert(at, kept);
        }
    }
    out
}

#[cfg(test)]
#[path = "raising_call_memory_tests.rs"]
mod tests;
