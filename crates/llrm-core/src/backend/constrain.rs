//! Port of `qbopt/backend/constrain.py`: giving a required register a value
//! of its own.
//!
//! The value an instruction requires in one register is a value of its own,
//! live across that instruction and nothing else, copied in and out.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

use iced_x86::Register;
use crate::support::hash::{IndexMap, IndexSet};

use crate::backend::{spiller, target};
use crate::model::ir::{self, Held, Loc, Mem, Operation, Semantics};
use crate::model::lir::{Insn, LirBlock, LirBody};

/// One value an instruction requires in two different registers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Impossible(pub String);

impl fmt::Display for Impossible {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Impossible {}

/// Whether two requirement names mean the same register at `width`.
fn _same_register(one: Register, other: Register, width: u32) -> bool {
    if one == other {
        return true;
    }
    let width = i64::from(width);
    if target::width_of(one) == Some(width) {
        return one == target::named(other, width);
    }
    if target::width_of(other) == Some(width) {
        return other == target::named(one, width);
    }
    ir::root(one) == ir::root(other)
}

/// Whether this value is pinned to the register the instruction wants.
fn _already_there(pinned: &IndexMap<u32, Register>, value: u32, register: Register, width: u32) -> bool {
    let Some(had) = pinned.get(&value) else {
        return false;
    };
    if ir::root(*had) != ir::root(register) {
        return false;
    }
    target::width_of(register) == Some(i64::from(width))
}

/// `widths.get(value) or _width(one, value)`.
fn _declared(widths: &IndexMap<u32, u32>, one: &Insn, value: u32) -> u32 {
    match widths.get(&value) {
        Some(width) if *width != 0 => *width,
        _ => _width(one, value),
    }
}

/// `body` with a value per required occurrence, and where each must live.
pub fn constrained(
    body: &LirBody,
    pinned: Option<&IndexMap<u32, Register>>,
) -> Result<(LirBody, IndexMap<u32, Register>), Impossible> {
    // A pin the caller released is not merged back from `body.pins`.
    let ids = body.pins.keys().chain(pinned.into_iter().flat_map(|given| given.keys())).copied().max();
    let mut pinned: IndexMap<u32, Register> = pinned.unwrap_or(&body.pins).clone();

    let defined: BTreeSet<u32> =
        body.blocks.iter().flat_map(|block| &block.insns).flat_map(|one| one.defines.iter().copied()).collect();
    let constants = spiller::_constants(body, &defined);

    let source = |value: u32, width: u32| -> Loc {
        match constants.get(&value) {
            Some(constant) if constant.width == width => Loc::Imm(constant.clone()),
            _ => Loc::Held(Held { value, width }),
        }
    };

    let mut fresh = _next_value(body).max(ids.unwrap_or(0) + 1);
    let mut pins: IndexMap<u32, Register> = IndexMap::default();
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut insns: Vec<Arc<Insn>> = Vec::new();
        for one in &block.insns {
            let mut one = Arc::clone(one);
            // CSE may feed several ABI slots from one value.
            let mut inputs: Vec<(Held, Register)> = Vec::new();
            let mut slots: IndexMap<(u32, u32, Register), Held> = IndexMap::default();
            let mut extra: Vec<u32> = Vec::new();
            for (held, register) in &one.requires {
                let key = (held.value, held.width, *register);
                if let Some(found) = slots.get(&key) {
                    inputs.push((*found, *register));
                    continue;
                }
                let distinct = if slots.keys().any(|(value, _width, _register)| *value == held.value) {
                    let distinct = Held { value: fresh, width: held.width };
                    insns.push(_move(&one, distinct, source(held.value, held.width)));
                    pins.insert(fresh, *register);
                    pinned.insert(fresh, *register);
                    extra.push(fresh);
                    fresh += 1;
                    distinct
                } else {
                    *held
                };
                slots.insert(key, distinct);
                inputs.push((distinct, *register));
            }
            if !extra.is_empty() {
                let mut made = (*one).clone();
                made.requires = inputs;
                made.uses.extend(extra);
                one = Arc::new(made);
            }
            let widths: IndexMap<u32, u32> =
                one.requires.iter().chain(&one.delivers).map(|(held, _r)| (held.value, held.width)).collect();
            let wanted: IndexMap<u32, (Register, Vec<(&'static str, usize)>)> = _wanted(&one)?
                .into_iter()
                .filter(|(value, got)| !_already_there(&pinned, *value, got.0, _declared(&widths, &one, *value)))
                .collect();
            let given: IndexMap<u32, Register> = _delivered(&one)
                .into_iter()
                .filter(|(value, register)| !_already_there(&pinned, *value, *register, _declared(&widths, &one, *value)))
                .collect();
            if wanted.is_empty() && given.is_empty() {
                insns.push(one);
                continue;
            }
            let mut before: Vec<Arc<Insn>> = Vec::new();
            let mut after: Vec<Arc<Insn>> = Vec::new();
            let mut swap: IndexMap<u32, u32> = IndexMap::default();
            let mut input_values: IndexMap<u32, u32> = IndexMap::default();
            let mut output_values: IndexMap<u32, u32> = IndexMap::default();
            let what = one.what.clone();
            let mut dests: Vec<Loc> = what.as_ref().map_or_else(Vec::new, |what| what.dests.clone());
            let mut sources: Vec<Loc> = what.as_ref().map_or_else(Vec::new, |what| what.sources.clone());
            let mut defines = one.defines.clone();
            let mut uses = one.uses.clone();
            let mut ordered: Vec<(u32, (Register, Vec<(&'static str, usize)>))> = wanted.into_iter().collect();
            ordered.sort_by_key(|(value, _got)| *value);
            for (value, (register, places)) in ordered {
                let held = Held { value: fresh, width: _declared(&widths, &one, value) };
                if places.is_empty() {
                    // No occurrence to rewrite: the instruction reads this in a
                    // register it names nowhere.
                    before.push(_move(&one, held, source(value, held.width)));
                    pins.insert(fresh, register);
                    uses = uses.iter().map(|v| if *v == value { held.value } else { *v }).collect();
                    swap.insert(value, held.value);
                    input_values.insert(value, held.value);
                    fresh += 1;
                    continue;
                }
                pins.insert(fresh, register);
                swap.insert(value, held.value);
                if places.iter().any(|(side, _)| *side == "source") {
                    before.push(_move(&one, held, source(value, held.width)));
                }
                if places.iter().any(|(side, _)| *side == "dest") {
                    after.push(_move(&one, Held { value, width: held.width }, Loc::Held(held)));
                }
                for (side, index) in places {
                    if side == "dest" {
                        dests[index] = Loc::Held(held);
                        defines = defines.iter().map(|v| if *v == value { held.value } else { *v }).collect();
                        output_values.insert(value, held.value);
                    } else {
                        sources[index] = Loc::Held(held);
                        uses = uses.iter().map(|v| if *v == value { held.value } else { *v }).collect();
                        input_values.insert(value, held.value);
                    }
                }
                fresh += 1;
            }
            let mut delivered: Vec<(u32, Register)> = given.into_iter().collect();
            delivered.sort_by_key(|(value, _register)| *value);
            for (value, register) in delivered {
                // Behind the instruction, not in front of it.
                let held = Held { value: fresh, width: _declared(&widths, &one, value) };
                after.push(_move(&one, Held { value, width: held.width }, Loc::Held(held)));
                pins.insert(fresh, register);
                defines = defines.iter().map(|v| if *v == value { held.value } else { *v }).collect();
                swap.insert(value, held.value);
                output_values.insert(value, held.value);
                fresh += 1;
            }
            insns.extend(before);

            let address = |operand: Loc| -> Loc {
                if !matches!(operand, Loc::Mem(_)) {
                    return operand;
                }
                ir::mapped(&operand, |value| Held {
                    value: swap.get(&value.value).copied().unwrap_or(value.value),
                    width: value.width,
                })
            };

            let rewritten = what.map(|what| Semantics {
                dests: dests.into_iter().map(address).collect(),
                sources: sources.into_iter().map(address).collect(),
                ..what
            });
            // A fixed-register requirement belongs to one occurrence.
            let explicit_uses = rewritten.as_ref().map_or_else(Vec::new, _semantic_reads);
            let mut made = (*one).clone();
            made.what = rewritten;
            made.defines = defines;
            made.uses = uses.into_iter().chain(explicit_uses).collect::<IndexSet<u32>>().into_iter().collect();
            made.requires = one
                .requires
                .iter()
                .map(|(held, register)| {
                    (
                        Held { value: input_values.get(&held.value).copied().unwrap_or(held.value), width: held.width },
                        *register,
                    )
                })
                .collect();
            made.delivers = one
                .delivers
                .iter()
                .map(|(held, register)| {
                    (
                        Held { value: output_values.get(&held.value).copied().unwrap_or(held.value), width: held.width },
                        *register,
                    )
                })
                .collect();
            insns.push(Arc::new(made));
            insns.extend(after);
        }
        blocks.push(block.with_insns(insns));
    }
    Ok((body.with_blocks(blocks), pins))
}

/// Where each value the body's instructions require has to live.
pub fn required(body: &LirBody) -> Result<IndexMap<u32, Register>, Impossible> {
    let mut out: IndexMap<u32, Register> = IndexMap::default();
    for block in &body.blocks {
        for one in &block.insns {
            for (value, (register, _where)) in _wanted(one)? {
                out.insert(value, register);
            }
            out.extend(_delivered(one));
        }
    }
    Ok(out)
}

/// Split address-class occurrences from an otherwise general value.
pub fn addressed(body: &LirBody, values: &BTreeSet<u32>) -> (LirBody, BTreeSet<u32>) {
    if values.is_empty() {
        return (body.clone(), BTreeSet::new());
    }

    let mut eligible: BTreeSet<u32> = values.clone();
    let mut occurrences: IndexMap<u32, i64> = values.iter().map(|value| (*value, 0)).collect();
    for block in &body.blocks {
        for one in &block.insns {
            let mut address: BTreeSet<u32> = BTreeSet::new();
            let mut ordinary: BTreeSet<u32> = BTreeSet::new();
            if let Some(what) = &one.what {
                for operand in &what.dests {
                    if let Loc::Mem(cell) = operand {
                        for held in [cell.base, cell.index].into_iter().flatten() {
                            address.insert(held.value);
                        }
                        if let Some(selector) = cell.selector {
                            ordinary.insert(selector.value);
                        }
                    }
                }
                for operand in &what.sources {
                    if let Loc::Mem(cell) = operand {
                        address.extend([cell.base, cell.index].into_iter().flatten().map(|held| held.value));
                        if let Some(selector) = cell.selector {
                            ordinary.insert(selector.value);
                        }
                    } else {
                        ordinary.extend(ir::values(operand).iter().map(|held| held.value));
                    }
                }
            }
            let hidden: BTreeSet<u32> = one.uses.iter().copied().filter(|value| !address.contains(value)).collect();
            let mut forbidden: BTreeSet<u32> = ordinary.union(&hidden).copied().collect();
            forbidden.extend(one.requires.iter().chain(&one.delivers).map(|(held, _register)| held.value));
            eligible.retain(|value| !forbidden.contains(value));
            if one.group.is_some() {
                eligible.retain(|value| !address.contains(value));
            }
            for value in address.intersection(values) {
                *occurrences.get_mut(value).expect("every value is counted") += 1;
            }
        }
    }

    let eligible: BTreeSet<u32> =
        eligible.into_iter().filter(|value| occurrences.get(value).copied().unwrap_or(0) > 1).collect();
    if eligible.is_empty() {
        return (body.clone(), BTreeSet::new());
    }

    let mut fresh = _next_value(body);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut insns: Vec<Arc<Insn>> = Vec::new();
        for one in &block.insns {
            let Some(what) = &one.what else {
                insns.push(Arc::clone(one));
                continue;
            };
            let mut widths: IndexMap<u32, u32> = IndexMap::default();
            for operand in what.dests.iter().chain(&what.sources) {
                let Loc::Mem(cell) = operand else {
                    continue;
                };
                for held in [cell.base, cell.index].into_iter().flatten() {
                    if eligible.contains(&held.value) {
                        widths.insert(held.value, held.width);
                    }
                }
            }
            if widths.is_empty() {
                insns.push(Arc::clone(one));
                continue;
            }
            let mut sorted: Vec<u32> = widths.keys().copied().collect();
            sorted.sort_unstable();
            let swap: IndexMap<u32, u32> =
                sorted.iter().enumerate().map(|(index, value)| (*value, fresh + index as u32)).collect();
            fresh += swap.len() as u32;
            for value in &sorted {
                insns.push(_move(
                    one,
                    Held { value: swap[value], width: widths[value] },
                    Loc::Held(Held { value: *value, width: widths[value] }),
                ));
            }

            let operand = |place: &Loc| -> Loc {
                let Loc::Mem(cell) = place else {
                    return place.clone();
                };
                let renamed = |held: Option<Held>| {
                    held.map(|held| Held { value: swap.get(&held.value).copied().unwrap_or(held.value), width: held.width })
                };
                Loc::Mem(Mem { base: renamed(cell.base), index: renamed(cell.index), ..cell.clone() })
            };

            let mut made = (**one).clone();
            made.what = Some(Semantics {
                dests: what.dests.iter().map(operand).collect(),
                sources: what.sources.iter().map(operand).collect(),
                ..what.clone()
            });
            made.uses = one.uses.iter().map(|value| swap.get(value).copied().unwrap_or(*value)).collect();
            insns.push(Arc::new(made));
        }
        blocks.push(block.with_insns(insns));
    }
    (body.with_blocks(blocks), eligible)
}

/// Each value this instruction writes in a register it names nowhere.
fn _delivered(one: &Insn) -> IndexMap<u32, Register> {
    one.delivers.iter().map(|(held, register)| (held.value, *register)).collect()
}

/// Each value this instruction requires somewhere, and where it sits.
fn _wanted(one: &Insn) -> Result<IndexMap<u32, (Register, Vec<(&'static str, usize)>)>, Impossible> {
    let mut out: IndexMap<u32, (Register, Vec<(&'static str, usize)>)> = IndexMap::default();
    for (held, register) in &one.requires {
        if let Some(found) = out.get(&held.value) {
            if !_same_register(found.0, *register, held.width) {
                return Err(Impossible(format!(
                    "{:#06x}: unsplit input value#{} requires two registers",
                    one.at, held.value
                )));
            }
        }
        out.insert(held.value, (*register, Vec::new()));
    }
    let Some(what) = &one.what else {
        return Ok(out);
    };
    for (index, operand) in what.sources.iter().enumerate() {
        if let Loc::Held(held) = operand {
            if let Some(found) = out.get_mut(&held.value) {
                found.1.push(("source", index));
            }
        }
    }
    for (place, register) in target::requirements(what) {
        let (side_name, side): (&'static str, &Vec<Loc>) =
            if place.side == "dest" { ("dest", &what.dests) } else { ("source", &what.sources) };
        if place.index >= side.len() {
            continue;
        }
        let Loc::Held(operand) = &side[place.index] else {
            continue;
        };
        let (held, mut places) = out.get(&operand.value).cloned().unwrap_or((register, Vec::new()));
        if !_same_register(held, register, operand.width) {
            return Err(Impossible(format!(
                "{:#06x}: value#{} is required in two registers at once",
                one.at, operand.value
            )));
        }
        if !places.contains(&(side_name, place.index)) {
            places.push((side_name, place.index));
        }
        out.insert(operand.value, (register, places));
    }
    Ok(out)
}

/// Values still read by a rewritten instruction's explicit operands.
fn _semantic_reads(what: &Semantics) -> Vec<u32> {
    let sources = what.sources.iter().flat_map(ir::values).map(|one| one.value);
    let addresses = what
        .dests
        .iter()
        .filter(|operand| !matches!(operand, Loc::Held(_)))
        .flat_map(ir::values)
        .map(|one| one.value);
    sources.chain(addresses).collect::<IndexSet<u32>>().into_iter().collect()
}

/// One copy, claiming none of the instruction's own bytes.
fn _move(beside: &Insn, into: Held, out_of: Loc) -> Arc<Insn> {
    let uses = match &out_of {
        Loc::Held(held) => vec![held.value],
        _ => Vec::new(),
    };
    let mut made = Insn::new(
        beside.at,
        Some((beside.at, beside.at)),
        Some(Semantics {
            name: Some("mov".to_owned()),
            dests: vec![Loc::Held(into)],
            sources: vec![out_of],
            ..Semantics::new(Operation::Move)
        }),
        vec![into.value],
        uses,
    );
    made.op = beside.op.clone();
    Arc::new(made)
}

fn _width(one: &Insn, value: u32) -> u32 {
    for (named, width) in &one.widths {
        if *named == value {
            return *width;
        }
    }
    let Some(what) = &one.what else {
        return 2;
    };
    for operand in what.dests.iter().chain(&what.sources) {
        if let Loc::Held(held) = operand {
            if held.value == value {
                return held.width;
            }
        }
    }
    2
}

fn _next_value(body: &LirBody) -> u32 {
    let mut every: Vec<u32> = body
        .blocks
        .iter()
        .flat_map(|block| &block.insns)
        .flat_map(|one| one.defines.iter().chain(&one.uses).copied())
        .collect();
    every.extend(body.blocks.iter().flat_map(|block| block.phis.iter().map(|phi| phi.result)));
    every.into_iter().max().unwrap_or(0) + 1
}

#[cfg(test)]
mod tests {
    //! Port of `tests/test_constrain.py`.
    use std::rc::Rc;
    use crate::model::mir::MirBody;

    use std::collections::BTreeSet;
    use std::sync::Arc;

    use iced_x86::Register;
    use crate::support::hash::IndexMap;

    use super::{addressed, constrained, required};
    use crate::backend::cpu::ProfileOrName;
    use crate::backend::frame::Frame;
    use crate::backend::{allocate, lower, select, spiller, target};
    use crate::model::ir::nodes::{Node, Opaque};
    use crate::model::ir::{self, Addr, Effects, Held, Imm, Loc, Mem, Operation, Reg, Semantics, Space, St};
    use crate::model::mir;
    use crate::model::lir::{Insn, LirBlock, LirBody};

    fn semantics(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> Semantics {
        Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) }
    }

    fn held(value: u32, width: u32) -> Loc {
        Loc::Held(Held { value, width })
    }

    fn imm(value: i64, width: u32) -> Loc {
        Loc::Imm(Imm { value, width, address: None })
    }

    fn _body(insns: Vec<Insn>) -> LirBody {
        LirBody::new(
            "one",
            0,
            vec![LirBlock::new(0, insns.into_iter().map(Arc::new).collect())],
            IndexMap::default(),
            IndexMap::default(),
        )
    }

    fn _insn(what: Semantics, defines: &[u32], uses: &[u32], at: i64) -> Insn {
        Insn::new(at, Some((at, at + 2)), Some(what), defines.to_vec(), uses.to_vec())
    }

    fn allocated(body: &LirBody, pins: &IndexMap<u32, Register>) -> allocate::Assignment {
        allocate::allocate(body, Some(pins), None, None, None, ProfileOrName::Name("386")).expect("allocates")
    }

    fn merged(one: &IndexMap<u32, Register>, other: &IndexMap<u32, Register>) -> IndexMap<u32, Register> {
        let mut out = one.clone();
        out.extend(other.iter().map(|(value, register)| (*value, *register)));
        out
    }

    fn what(one: &Insn) -> &Semantics {
        one.what.as_ref().expect("semantics")
    }

    fn pinned(pairs: &[(u32, Register)]) -> IndexMap<u32, Register> {
        pairs.iter().copied().collect()
    }

    /// H_BENCH printed ft_n=0 and infinite ft_mean: SI's result was spilled from BX.
    #[test]
    fn test_call_result_spill_keeps_its_register_after_input_split() {
        let (argument, result) = (Held { value: 1, width: 2 }, Held { value: 2, width: 2 });
        let source = _insn(semantics(Operation::Move, "mov", vec![Loc::Held(argument)], vec![imm(7, 2)]), &[1], &[], 0x100);
        let mut call = _insn(semantics(Operation::Call, "call", vec![], vec![]), &[2], &[1], 0x108);
        call.requires = vec![(argument, Register::AX)];
        call.delivers = vec![(result, Register::SI)];
        let use_ =
            _insn(semantics(Operation::Move, "mov", vec![held(3, 2)], vec![Loc::Held(result)]), &[3], &[2], 0x110);
        let (split, pins) =
            constrained(&_body(vec![source, call, use_]), Some(&pinned(&[(2, Register::SI)]))).unwrap();
        let mut frame = Frame::new(0);
        let (spilled, _) = spiller::spilled(&split, &BTreeSet::from([2]), Some(&mut frame)).unwrap();
        let assignment = allocated(&spilled, &merged(&pins, &required(&spilled).unwrap()));
        let placed = allocate::applied(&spilled, &assignment).unwrap();
        let insns = placed.insns();
        let call_index = insns.iter().position(|one| what(one).op == Operation::Call).unwrap();
        let store = &insns[call_index + 1];
        assert!(matches!(what(store).dests[0], Loc::Mem(_)));
        assert_eq!(what(store).sources, vec![Loc::Reg(Reg { register: Register::SI, width: 2 })]);
    }

    /// JUMPS refused emission: ON GOTO read old v20 while its required input became v143.
    #[test]
    fn test_runtime_requirement_renames_its_explicit_source() {
        let value = Held { value: 20, width: 2 };
        let mut call = _insn(semantics(Operation::Call, "call", vec![], vec![Loc::Held(value)]), &[], &[20], 0x100);
        call.requires = vec![(value, Register::AX)];
        let (got, _) = constrained(&_body(vec![call]), None).unwrap();
        let result = got.insns().into_iter().find(|one| what(one).op == Operation::Call).unwrap();
        let Loc::Held(source) = what(&result).sources[0] else { panic!("not a value") };
        assert_eq!(source.value, result.uses[0]);
    }

    /// QGLDIFF hung in heap compaction: ENRA lost BX=0 after CSE joined its CX=0.
    #[test]
    fn test_shared_zero_is_supplied_to_every_runtime_input() {
        for registers in [vec![Register::BX, Register::CX], vec![Register::AX, Register::BX, Register::CX]] {
            let value = Held { value: 1, width: 2 };
            let constant =
                _insn(semantics(Operation::Move, "mov", vec![Loc::Held(value)], vec![imm(0, 2)]), &[1], &[], 0x100);
            let mut call = _insn(semantics(Operation::Call, "call", vec![], vec![]), &[], &[1], 0x108);
            call.requires = registers.iter().map(|register| (value, *register)).collect();
            let (body, pins) = constrained(&_body(vec![constant, call]), None).unwrap();
            let placed = allocate::applied(&body, &allocated(&body, &pins)).unwrap();
            let zeros: BTreeSet<Register> = placed
                .insns()
                .iter()
                .filter(|one| what(one).name.as_deref() == Some("mov") && what(one).sources == vec![imm(0, 2)])
                .map(|one| match &what(one).dests[0] {
                    Loc::Reg(reg) => reg.register,
                    other => panic!("not a register: {other:?}"),
                })
                .collect();
            let wanted: BTreeSet<Register> = registers.iter().copied().collect();
            assert!(wanted.is_subset(&zeros), "{registers:?}: {zeros:?}");
            let last = body.insns().last().cloned().unwrap();
            let got: BTreeSet<Register> = last.uses.iter().map(|value| pins[value]).collect();
            assert_eq!(got, wanted);
        }
    }

    /// Qrender FIDIV 035c retained unplaced v398 after its SI input became v1036.
    #[test]
    fn test_fixed_address_requirement_renames_memory_base() {
        let value = Held { value: 20, width: 2 };
        let cell = Mem { base: Some(value), ..Mem::new(Some(Addr::new(Space::Literal, 0)), 2) };
        let mut instruction = _insn(
            semantics(
                Operation::FloatArith,
                "fidiv",
                vec![Loc::St(St { index: 0 })],
                vec![Loc::St(St { index: 0 }), Loc::Mem(cell)],
            ),
            &[],
            &[20],
            0x100,
        );
        instruction.requires = vec![(value, Register::SI)];
        let (got, pins) = constrained(&_body(vec![instruction]), None).unwrap();
        let result = got.insns().last().cloned().unwrap();
        let Some(Loc::Mem(cell)) = what(&result).sources.last() else { panic!("not memory") };
        assert_eq!(cell.base.unwrap().value, result.uses[0]);
        assert_eq!(pins[&result.uses[0]], Register::SI);
        let placed = allocate::applied(&got, &allocated(&got, &pins)).unwrap();
        let emitted = select::emit(what(placed.insns().last().unwrap()), 0, None, false, false, None);
        assert!(emitted.is_some());
        assert_eq!(emitted.unwrap().code, [0xDE, 0x34]); // fidiv word [si]
    }

    /// D_SURF returned sc_test=-4000: spilling ES left a far load on the old segment.
    #[test]
    fn test_spilled_segment_load_still_sets_es() {
        let segment = mir::Value::new(1, 0xFCC);
        let cell = mir::Arg::Cell(mir::Cell { r#ref: mir::MemRef::new(Some(Addr::new(Space::Frame, -2)), 2) });
        let mut op = mir::Op::new(0xFCC, mir::OpCode::Operation(Operation::Move), "mov", vec![segment], vec![]);
        op.kind = mir::Kind::Load;
        op.args = vec![cell];
        op.results = vec![mir::Arg::Held(mir::Held { value: segment, width: 2 })];
        let context = mir::MirBody::new(0xFCC, vec![mir::MirBlock::new(0xFCC, vec![], vec![op.clone()], vec![])]);
        let calls = IndexMap::default();
        let contracts = IndexMap::default();
        let options = lower::Options { origin: [(segment, Register::ES)].into_iter().collect(), ..Default::default() };
        let mut lowering = lower::Lowering::new(
            &Rc::new(MirBody::clone(&context)),
            BTreeSet::from([segment.id]),
            &calls,
            BTreeSet::new(),
            Some(&contracts),
            "386",
            options,
        )
        .unwrap();
        let expanded = lowering.expand(&op, true).unwrap();
        let [load] = expanded.as_slice() else { panic!("{} instructions", expanded.len()) };
        let far = Mem {
            selector: Some(Held { value: segment.id, width: 2 }),
            ..Mem::new(Some(Addr { segment: Register::ES, ..Addr::new(Space::Far, 0x10) }), 2)
        };
        let use_ =
            _insn(semantics(Operation::Move, "mov", vec![Loc::Mem(far)], vec![imm(7, 2)]), &[], &[segment.id], 0xFCF);
        let (body, pins) = constrained(&_body(vec![(**load).clone(), use_]), Some(&IndexMap::default())).unwrap();
        let (spilled, _) = spiller::spilled(&body, &BTreeSet::from([segment.id]), Some(&mut Frame::new(0))).unwrap();
        let wanted = merged(&pins, &required(&spilled).unwrap());
        let placed = allocate::applied(&spilled, &allocated(&spilled, &wanted)).unwrap();
        let decoded: Vec<iced_x86::Instruction> = placed
            .insns()
            .iter()
            .map(|one| {
                let code = select::emit(what(one), 0, None, false, false, None).expect("encodes").code;
                iced_x86::Decoder::new(16, &code, iced_x86::DecoderOptions::NONE).decode()
            })
            .collect();
        let [reload, access] = decoded.as_slice() else { panic!("{} instructions", decoded.len()) };
        assert!(target::SELECTORS.contains(&reload.op0_register()));
        assert_eq!(
            access.segment_prefix(),
            reload.op0_register(),
            "the far access reads a segment the reload did not set"
        );
    }

    /// D_SURF sc_test=-4000: a reused slot selector read through the LRU array's ES.
    ///
    /// Only `selected_site=False`: the selected site is `Lowering`'s dict form of
    /// `absorbed`, which is not ported.
    #[test]
    fn test_far_read_restores_its_forwarded_selector() {
        let (segment, result) = (mir::Value::new(1, 0), mir::Value::new(2, 8));
        let addr = Addr { segment: Register::ES, ..Addr::new(Space::Far, 0) };
        let reference = mir::MemRef { segment: Some(segment), ..mir::MemRef::new(Some(addr), 2) };
        let machine = semantics(
            Operation::Move,
            "mov",
            vec![Loc::Reg(Reg { register: Register::AX, width: 2 })],
            vec![Loc::Mem(Mem { through: Register::BX, ..Mem::new(Some(addr), 2) })],
        );
        let mut op = mir::Op::new(8, mir::OpCode::Operation(Operation::Move), "mov", vec![result], vec![segment]);
        op.kind = mir::Kind::Load;
        op.args = vec![mir::Arg::Cell(mir::Cell { r#ref: reference.clone() })];
        op.results = vec![mir::Arg::Held(mir::Held { value: result, width: 2 })];
        op.loads = vec![reference];
        op.source_backed = true;
        op.source = Some(8);
        op.raised = Some((vec![], vec![]));
        let context = mir::MirBody::new(0, vec![mir::MirBlock::new(0, vec![], vec![op.clone()], vec![])]);
        // Python's `SimpleNamespace(semantics=machine)`: only the semantics is read.
        let node = Node::Opaque(Opaque { semantics: machine, ..Opaque::new(_any_insn(), Effects::no_effect()) });
        let calls = IndexMap::default();
        let contracts = IndexMap::default();
        let options = lower::Options {
            nodes: [(8, Arc::new(node))].into_iter().collect(),
            origin: [(segment, Register::ES)].into_iter().collect(),
            ..Default::default()
        };
        let mut lowering = lower::Lowering::new(
            &Rc::new(MirBody::clone(&context)),
            BTreeSet::from([1, 2]),
            &calls,
            BTreeSet::new(),
            Some(&contracts),
            "386",
            options,
        )
        .unwrap();
        let expanded = lowering.expand(&op, true).unwrap();
        let [read] = expanded.as_slice() else { panic!("{} instructions", expanded.len()) };
        assert!(read.uses.contains(&segment.id), "the selector must remain live until the far read");
        let saved = _insn(
            semantics(
                Operation::Move,
                "mov",
                vec![held(1, 2)],
                vec![Loc::Mem(Mem { through: Register::BP, ..Mem::new(Some(Addr::new(Space::Frame, -2)), 2) })],
            ),
            &[1],
            &[],
            0x100,
        );
        let overwrite = _insn(
            semantics(
                Operation::Move,
                "mov",
                vec![Loc::Reg(Reg { register: Register::ES, width: 2 })],
                vec![Loc::Reg(Reg { register: Register::DX, width: 2 })],
            ),
            &[],
            &[],
            4,
        );
        let cx = pinned(&[(1, Register::CX)]);
        let body = allocate::explicit_selectors(&_body(vec![saved, overwrite, (**read).clone()]), Some(&cx));
        let (body, pins) = constrained(&body, Some(&cx)).unwrap();
        let placed = allocate::applied(&body, &allocated(&body, &merged(&cx, &pins))).unwrap();
        let insns = placed.insns();
        let restore = what(&insns[insns.len() - 2]);
        assert_eq!(restore.dests, [Loc::Reg(Reg { register: Register::ES, width: 2 })]);
        assert_eq!(restore.sources, [Loc::Reg(Reg { register: Register::CX, width: 2 })]);
    }

    fn _any_insn() -> crate::frontends::bc::declen::Insn {
        crate::frontends::bc::declen::decode(&[0x89, 0xC0], 0).unwrap()
    }

    fn _shift(count: u32) -> Insn {
        let what = semantics(Operation::Binary, "shl", vec![held(9, 2)], vec![held(9, 2), held(count, 2)]);
        _insn(what, &[9], &[9, count], 0x100)
    }

    fn _extend(one: u32, into: u32) -> Insn {
        _insn(semantics(Operation::Extend, "cwd", vec![held(into, 2)], vec![held(one, 2)]), &[into], &[one], 0x100)
    }

    /// Nbody kept 512 in EDI while copying it into EAX, displacing its accumulator.
    #[test]
    fn test_fixed_input_rematerializes_constant_without_retaining_source() {
        let constant = _insn(semantics(Operation::Move, "mov", vec![held(1, 2)], vec![imm(512, 2)]), &[1], &[], 0x100);
        let (done, pins) = constrained(&_body(vec![constant, _extend(1, 2)]), None).unwrap();
        let prepared = done
            .insns()
            .into_iter()
            .find(|one| !one.defines.is_empty() && pins.get(&one.defines[0]) == Some(&Register::EAX))
            .unwrap();
        assert_eq!(what(&prepared).sources, vec![imm(512, 2)]);
        assert!(prepared.uses.is_empty());
    }

    fn _multiply(low: u32, high: u32, by: u32) -> Insn {
        let what =
            semantics(Operation::Multiply, "imul", vec![held(low, 2), held(high, 2)], vec![held(low, 2), held(by, 2)]);
        _insn(what, &[low, high], &[low, by], 0x100)
    }

    fn _shape(body: &LirBody) -> Vec<String> {
        body.insns()
            .iter()
            .map(|one| format!("{:?} {:?} <- {:?}", what(one).name, what(one).dests, what(one).sources))
            .collect()
    }

    /// A shift counts from cl and names it nowhere.
    #[test]
    fn test_a_required_source_is_copied_in_before_the_instruction() {
        let (got, pins) = constrained(&_body(vec![_shift(7)]), None).unwrap();
        assert_eq!(got.blocks[0].insns.len(), 2);
        let (first, then) = (&got.blocks[0].insns[0], &got.blocks[0].insns[1]);
        assert!(what(first).name.as_deref() == Some("mov") && what(first).sources == vec![held(7, 2)]);
        let Loc::Held(fresh) = what(first).dests[0] else { panic!("not a value") };
        assert_eq!(what(then).sources[1], held(fresh.value, 2));
        assert_eq!(pins, pinned(&[(fresh.value, Register::ECX)]));
        assert!(!pins.contains_key(&7), "the original keeps no hardware requirement");
    }

    /// `cwd` writes dx and names it nowhere.
    #[test]
    fn test_a_required_destination_is_copied_out_after_the_instruction() {
        let (got, pins) = constrained(&_body(vec![_extend(1, 8)]), None).unwrap();
        let insns = &got.blocks[0].insns;
        assert_eq!(insns.len(), 3, "{:?}", _shape(&got));
        let last = insns.last().unwrap();
        assert!(what(last).name.as_deref() == Some("mov") && what(last).dests == vec![held(8, 2)]);
        let Loc::Held(fresh) = what(last).sources[0] else { panic!("not a value") };
        assert!(pins[&fresh.value] == Register::EDX && !pins.contains_key(&8));
    }

    /// `imul`'s low half is its first source and its first destination.
    /// Two fresh values would name two registers and the tie would be gone.
    #[test]
    fn test_a_tied_source_and_destination_share_one_fresh_value() {
        let (got, pins) = constrained(&_body(vec![_multiply(1, 2, 3)]), None).unwrap();
        let middle: Vec<_> =
            got.blocks[0].insns.iter().filter(|one| what(one).name.as_deref() == Some("imul")).collect();
        assert_eq!(middle.len(), 1);
        let (low_in, low_out) = (&what(middle[0]).sources[0], &what(middle[0]).dests[0]);
        assert_eq!(low_in, low_out, "the tie was broken");
        let Loc::Held(low_in) = low_in else { panic!("not a value") };
        assert_eq!(pins[&low_in.value], Register::EAX);
        let Loc::Held(high) = what(middle[0]).dests[1] else { panic!("not a value") };
        assert_eq!(pins[&high.value], Register::EDX);
        assert!(!pins.contains_key(&1) && !pins.contains_key(&2));
    }

    /// Nib nbody could not allocate ``fixed_mul(x, x)``.
    #[test]
    fn test_a_value_remains_live_at_unconstrained_occurrences_of_the_same_instruction() {
        let repeated = held(7, 4);
        let multiply =
            semantics(Operation::Multiply, "imul", vec![held(8, 4), held(9, 4)], vec![repeated.clone(), repeated.clone()]);
        let (got, pins) = constrained(&_body(vec![_insn(multiply, &[8, 9], &[7], 0x100)]), None).unwrap();
        let multiply = got.insns().into_iter().find(|one| what(one).name.as_deref() == Some("imul")).unwrap();
        let Loc::Held(fixed) = what(&multiply).sources[0] else { panic!("not a value") };
        assert_eq!(pins[&fixed.value], Register::EAX);
        assert_eq!(what(&multiply).sources[1], repeated);
        let uses: BTreeSet<u32> = multiply.uses.iter().copied().collect();
        assert_eq!(uses, BTreeSet::from([fixed.value, 7]));
    }

    /// A value that is both a shift's count and a multiply's high half
    /// cannot be placed at all.
    #[test]
    fn test_one_value_required_in_two_registers_is_refused() {
        let multiply =
            semantics(Operation::Multiply, "imul", vec![held(1, 2), held(7, 2)], vec![held(7, 2), held(3, 2)]);
        let error = constrained(&_body(vec![_insn(multiply, &[1, 7], &[7, 3], 0x100)]), None).unwrap_err();
        assert!(error.0.contains("two registers"), "{error}");
    }

    /// PROCS-P-OT crashed rebuilding TWICE's `rep stosw`.
    #[test]
    fn test_an_explicit_word_requirement_and_its_root_are_one_register() {
        let (value, count, address) =
            (Held { value: 11, width: 2 }, Held { value: 8, width: 2 }, Held { value: 9, width: 2 });
        let fill = semantics(
            Operation::Fill,
            "stosw",
            vec![Loc::Mem(Mem::new(None, 0)), held(12, 2), held(13, 2)],
            vec![
                Loc::Held(value),
                Loc::Held(count),
                Loc::Held(address),
                Loc::Reg(Reg { register: Register::ES, width: 2 }),
            ],
        );
        let mut fill = Insn::new(0x104, Some((0x104, 0x106)), Some(fill), vec![12, 13], vec![11, 8, 9]);
        fill.requires = vec![(value, Register::AX), (count, Register::CX), (address, Register::DI)];
        let (got, pins) = constrained(&_body(vec![fill]), None).unwrap();
        let filled = got.insns().into_iter().find(|one| what(one).op == Operation::Fill).unwrap();
        assert_eq!(what(&filled).sources.len(), 4);
        let roots: BTreeSet<Register> = pins.values().map(|register| ir::root(*register)).collect();
        assert_eq!(roots, BTreeSet::from([Register::EAX, Register::ECX, Register::EDI]));
    }

    #[test]
    fn test_the_helper_moves_belong_to_no_parallel_copy() {
        let (got, _pins) = constrained(&_body(vec![_shift(7)]), None).unwrap();
        assert!(got.blocks[0].insns.iter().all(|one| one.group.is_none()));
        assert_eq!(got.blocks[0].insns[0].covers, Some((0x100, 0x100)), "a helper claims no bytes");
    }

    /// A runtime routine takes its arguments in fixed registers and names
    /// none of them. arrprm's B$ENRA wanted cx and bx and got si and di.
    #[test]
    fn test_a_value_a_call_reads_in_a_register_it_names_nowhere() {
        let call = semantics(Operation::Call, "call", vec![], vec![]);
        let mut call = Insn::new(0x106, Some((0x106, 0x10B)), Some(call), vec![11], vec![2]);
        call.requires = vec![(Held { value: 2, width: 2 }, Register::CX)];
        let (got, pins) = constrained(&_body(vec![call]), None).unwrap();
        let insns = &got.blocks[0].insns;
        assert_eq!(insns.len(), 2, "expected one copy before the call");
        let (moved, after) = (&insns[0], &insns[1]);
        assert!(what(moved).op == Operation::Move && what(moved).sources == vec![held(2, 2)]);
        let fresh = moved.defines[0];
        assert_eq!(pins, pinned(&[(fresh, Register::CX)]));
        assert!(!pins.contains_key(&2), "the argument itself was pinned");
        assert_eq!(after.uses, vec![fresh], "the call still reads the argument");
        assert_eq!(
            after.requires,
            vec![(Held { value: fresh, width: 2 }, Register::CX)],
            "a later spill must retain the ABI slot"
        );

        let (again, more) = constrained(&got, Some(&pins)).unwrap();
        assert!(again == got && more.is_empty(), "constraining twice is not constraining once");
    }

    /// The split was supposed to free the value, and made it unplaceable.
    #[test]
    fn test_a_value_already_in_the_register_a_call_needs_is_not_split() {
        let call = semantics(Operation::Call, "call", vec![], vec![]);
        let mut call = Insn::new(0x10, Some((0x10, 0x15)), Some(call), vec![], vec![11]);
        call.requires = vec![(Held { value: 11, width: 2 }, Register::CX)];
        let (got, pins) = constrained(&_body(vec![call]), Some(&pinned(&[(11, Register::ECX)]))).unwrap();
        let insns = &got.blocks[0].insns;
        assert_eq!(insns.len(), 1, "a copy was inserted for a value already there");
        assert_eq!(insns[0].uses, vec![11]);
        assert!(pins.get(&11).is_none_or(|register| *register == Register::ECX), "re-pinned: {pins:?}");
        assert!(
            !pins.iter().any(|(value, register)| *register == Register::CX && *value != 11),
            "a fresh value was pinned onto the same register: {pins:?}"
        );
    }

    /// The declared width is the only statement an idiom makes of one.
    #[test]
    fn test_a_value_already_in_the_register_an_idiom_wants_is_not_copied() {
        let restore = semantics(Operation::Restore, "restore", vec![], vec![]);
        let mut one = Insn::new(0x10, Some((0x10, 0x14)), Some(restore), vec![], vec![2]);
        one.requires = vec![(Held { value: 2, width: 4 }, Register::EAX)];
        let body = LirBody::new(
            "one",
            0x10,
            vec![LirBlock::new(0x10, vec![Arc::new(one)])],
            IndexMap::default(),
            IndexMap::default(),
        );
        let (got, fixed) = constrained(&body, Some(&pinned(&[(2, Register::EAX)]))).unwrap();
        assert!(fixed.is_empty(), "a copy was minted for a value already in eax: {fixed:?}");
        assert_eq!(got.insns()[0].uses, vec![2], "the instruction was given a fresh value it did not need");
    }

    fn read(at: i64, result: u32, selector: Option<Held>) -> Insn {
        let cell = Mem { base: Some(Held { value: 1, width: 2 }), selector, ..Mem::new(Some(Addr::new(Space::Far, at)), 2) };
        _insn(semantics(Operation::Move, "mov", vec![held(result, 2)], vec![Loc::Mem(cell)]), &[result], &[1], at)
    }

    /// lru_use stored long-lived address values instead of making short copies.
    #[test]
    fn test_address_class_is_split_at_each_constrained_occurrence() {
        let source = _insn(semantics(Operation::Move, "mov", vec![held(1, 2)], vec![imm(7, 2)]), &[1], &[], 1);
        let (split, opened) = addressed(&_body(vec![source, read(2, 2, None), read(3, 3, None)]), &BTreeSet::from([1]));
        let insns = split.insns();
        let copies: Vec<_> =
            insns.iter().filter(|one| what(one).name.as_deref() == Some("mov") && one.uses == vec![1]).collect();
        let confined = allocate::classes(&split, &BTreeSet::new());

        assert_eq!(opened, BTreeSet::from([1]));
        assert_eq!(copies.len(), 2);
        assert!(!confined.contains_key(&1));
        assert!(copies.iter().all(|one| confined[&one.defines[0]].is_subset(&target::ADDRESSING)));
    }

    /// A base and far selector sharing one value cannot be partially renamed.
    #[test]
    fn test_address_occurrence_split_refuses_a_value_also_used_as_selector() {
        let owner = _insn(semantics(Operation::Move, "mov", vec![held(1, 2)], vec![imm(7, 2)]), &[1], &[], 1);
        let selector = Some(Held { value: 1, width: 2 });
        let body = _body(vec![owner, read(2, 2, selector), read(3, 3, selector)]);
        let (split, opened) = addressed(&body, &BTreeSet::from([1]));

        assert_eq!(split, body);
        assert!(opened.is_empty());
    }
}
