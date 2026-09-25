//! Dynamic arrays laid out end to end in one far allocation.
//!
//! A dynamic array whose descriptor nothing reads but its own elements --
//! one DIM with constant bounds, no REDIM, never handed on whole, only its
//! selector, origin and shape read -- has no location the program can see.
//! Arrays the same blocks read are placed in one allocation of at most 64K:
//! one selector serves them all, and an element's offset is the allocation's
//! origin plus a constant plus its scaled subscripts. Same-shape arrays some
//! block reads at one subscript are interleaved as one array of records, so
//! a loop walks them all with one pointer. The first member's descriptor
//! describes the allocation; the others' are never filled.

use std::collections::{BTreeMap, BTreeSet};

use super::shapes::{self, Identity};
use super::tags::{Shape, Slot, Tag};
use super::{Compiler, Function, Instruction, Number, Operand, INTEGER, STRING};

/// The most one far array holds (rt/dynamic.asm B$DDIM).
const SEGMENT: i64 = 1 << 16;

/// A candidate's DIM and constant shape.
#[derive(Clone)]
struct Member {
    /// Where its DIM runs: function, block, instruction.
    function: usize,
    block: usize,
    position: usize,
    /// Each descriptor record's bounds, record 0 first.
    records: Vec<(i64, i64)>,
    width: i64,
}

impl Member {
    fn count(&self) -> i64 {
        self.records.iter().map(|(lower, upper)| upper - lower + 1).product()
    }

    fn bytes(&self) -> i64 {
        self.count() * self.width
    }

    fn align(&self) -> i64 {
        self.width.min(4)
    }

    /// The element number of every lower bound, as the element formula
    /// numbers it: what B$DDIM subtracts from the data offset.
    fn adjustment(&self) -> i64 {
        self.records.iter().fold(0, |sum, (lower, upper)| sum * (upper - lower + 1) + lower)
    }
}

/// Every value an operand reads.
fn read(operand: &Operand) -> Vec<u32> {
    match operand {
        Operand::Value(value) => vec![*value],
        Operand::Indirect { base, .. } => vec![*base],
        Operand::Element(_, indices) | Operand::Projection { indices, .. } => indices.iter().flat_map(read).collect(),
        Operand::Constant(..) | Operand::Place(_) => Vec::new(),
    }
}

/// The places an operand names.
fn named(operand: &Operand) -> Vec<u32> {
    match operand {
        Operand::Place(place) | Operand::Element(place, _) | Operand::Projection { place, .. } => vec![*place],
        _ => Vec::new(),
    }
}

fn constant(operand: &Operand) -> Option<i64> {
    match operand {
        Operand::Constant(_, Number::Integer(value)) => Some(*value),
        _ => None,
    }
}

fn rounded(at: i64, align: i64) -> i64 {
    (at + align - 1) / align * align
}

/// A value's computation spelled out, so equal subscripts compare equal;
/// None when it is too deep to be worth comparing.
fn spelled(defining: &BTreeMap<u32, &Instruction>, value: u32, depth: usize) -> Option<String> {
    let Some(one) = defining.get(&value) else {
        return Some(format!("v{value}"));
    };
    if depth > 8 {
        return None;
    }
    if let Some(Tag::DescriptorField { field, .. }) = &one.tag {
        return Some(format!("{field:?}"));
    }
    let operands: Option<Vec<String>> = one
        .operands
        .iter()
        .map(|operand| match operand {
            Operand::Value(inner) => spelled(defining, *inner, depth + 1),
            Operand::Constant(_, Number::Integer(value)) => Some(value.to_string()),
            Operand::Place(place) => Some(format!("p{place}")),
            _ => None,
        })
        .collect();
    Some(format!("{}({})", one.op, operands?.join(",")))
}

/// What the module lets be merged, and which arrays each block reads.
struct Survey {
    members: BTreeMap<Identity, Member>,
    /// Per block, the arrays whose elements it reads or writes.
    accessed: Vec<BTreeSet<Identity>>,
    /// Per block, the arrays it releases.
    released: Vec<BTreeSet<Identity>>,
    /// Pairs some block reads at one subscript, lesser first.
    walked: BTreeSet<(Identity, Identity)>,
    /// Arrays with an access not scaled by a constant: they keep their stride.
    unstrided: BTreeSet<Identity>,
}

fn surveyed(compiler: &Compiler) -> Survey {
    let mut members: BTreeMap<Identity, Member> = BTreeMap::new();
    let mut allocations: BTreeMap<Identity, usize> = BTreeMap::new();
    let mut refused: BTreeSet<Identity> = BTreeSet::new();
    let (mut accessed, mut released) = (Vec::new(), Vec::new());
    let (mut walked, mut unstrided) = (BTreeSet::new(), BTreeSet::new());
    for (index, function) in compiler.functions.iter().enumerate() {
        let pointers = shapes::pointers(function);
        let defining: BTreeMap<u32, &Instruction> = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .flat_map(|one| one.results.iter().map(move |value| (*value, one)))
            .collect();
        let places: BTreeMap<u32, Identity> =
            function.places.iter().map(|place| (place.id, shapes::identity(function, place))).collect();
        refused.extend(pointers.values().filter(|identity| matches!(identity, Identity::Parameter(..))));
        refused.extend(function.places.iter().filter(|place| place.storage == "external").map(|place| places[&place.id]));
        for (at, block) in function.blocks.iter().enumerate() {
            let (mut touched, mut freed) = (BTreeSet::new(), BTreeSet::new());
            let mut subscripts: BTreeMap<String, BTreeSet<Identity>> = BTreeMap::new();
            for (position, one) in block.instructions.iter().enumerate() {
                // The descriptor pointer this instruction may read as a member's.
                let allowed = match &one.tag {
                    Some(Tag::DescriptorField { descriptor, field }) if one.op == "load" && *field != Slot::Data => Some(*descriptor),
                    Some(Tag::Allocate(shape)) => {
                        if let Some(identity) = pointers.get(&shape.descriptor) {
                            *allocations.entry(*identity).or_default() += 1;
                            let records: Option<Vec<(i64, i64)>> =
                                shape.records.iter().map(|(lower, upper)| Some((constant(lower)?, constant(upper)?))).collect();
                            match records.filter(|_| shape.element != STRING) {
                                Some(records) => {
                                    let width = compiler.width(shape.element) as i64;
                                    members.insert(*identity, Member { function: index, block: at, position, records, width });
                                }
                                None => {
                                    refused.insert(*identity);
                                }
                            }
                        }
                        Some(shape.descriptor)
                    }
                    Some(Tag::Release { descriptor }) => {
                        freed.extend(pointers.get(descriptor));
                        Some(*descriptor)
                    }
                    Some(Tag::Reallocate(shape)) => {
                        refused.extend(pointers.get(&shape.descriptor));
                        None
                    }
                    Some(Tag::ElementOffset { descriptor, origin }) => {
                        let Some(identity) = pointers.get(descriptor) else {
                            continue;
                        };
                        // Only `origin + bytes` takes a constant between them.
                        let split = one.op == "add"
                            && matches!((origin, one.operands.as_slice()), (Some(origin), [Operand::Value(first), Operand::Value(_)]) if first == origin);
                        if split {
                            touched.insert(*identity);
                            // `bytes` is `subscript * width`.
                            let scaled = match one.operands[1] {
                                Operand::Value(bytes) => defining.get(&bytes).filter(|mul| mul.op == "mul"),
                                _ => None,
                            };
                            match scaled.map(|mul| mul.operands.as_slice()) {
                                Some([subscript, Operand::Constant(..)]) => {
                                    let spelling = match subscript {
                                        Operand::Value(subscript) => spelled(&defining, *subscript, 0),
                                        other => constant(other).map(|value| value.to_string()),
                                    };
                                    if let Some(spelling) = spelling {
                                        subscripts.entry(spelling).or_default().insert(*identity);
                                    }
                                }
                                _ => {
                                    unstrided.insert(*identity);
                                }
                            }
                        } else {
                            refused.insert(*identity);
                        }
                        None
                    }
                    _ => None,
                };
                for operand in &one.operands {
                    for value in read(operand).into_iter().filter(|value| Some(*value) != allowed) {
                        refused.extend(pointers.get(&value));
                    }
                    if one.op != "address" {
                        refused.extend(named(operand).iter().filter_map(|place| places.get(place)));
                    }
                }
            }
            for operand in block.terminator.iter().flat_map(|one| &one.operands) {
                refused.extend(read(operand).iter().filter_map(|value| pointers.get(value)));
                refused.extend(named(operand).iter().filter_map(|place| places.get(place)));
            }
            for together in subscripts.values() {
                for one in together {
                    walked.extend(together.range(..*one).map(|other| (*other, *one)));
                }
            }
            accessed.push(touched);
            released.push(freed);
        }
        // An element's address used as anything but a load or store's base
        // shows code outside the stride: its array keeps its own.
        let mut elements: BTreeMap<u32, Identity> = BTreeMap::new();
        for one in function.blocks.iter().flat_map(|block| &block.instructions) {
            match (&one.tag, one.op, one.operands.as_slice()) {
                (Some(Tag::ElementOffset { descriptor, .. }), ..) => {
                    if let Some(identity) = pointers.get(descriptor) {
                        elements.extend(one.results.iter().map(|result| (*result, *identity)));
                    }
                }
                (_, "concat", [_, Operand::Value(offset)]) => {
                    if let Some(identity) = elements.get(offset).copied() {
                        elements.extend(one.results.iter().map(|result| (*result, identity)));
                    }
                }
                _ => {}
            }
        }
        for block in &function.blocks {
            for one in &block.instructions {
                let offset = match (one.op, one.operands.as_slice()) {
                    ("concat", [_, Operand::Value(offset)]) => Some(*offset),
                    _ => None,
                };
                for operand in &one.operands {
                    match operand {
                        Operand::Value(value) if Some(*value) != offset => unstrided.extend(elements.get(value)),
                        // A call's by-reference argument hands the address on.
                        Operand::Indirect { base, .. } if one.callee.is_some() => unstrided.extend(elements.get(base)),
                        _ => {}
                    }
                }
            }
            for operand in block.terminator.iter().flat_map(|one| &one.operands) {
                if let Operand::Value(value) = operand {
                    unstrided.extend(elements.get(value));
                }
            }
        }
    }
    members.retain(|identity, _| allocations[identity] == 1 && !refused.contains(identity));
    Survey { members, accessed, released, walked, unstrided }
}

/// Union-find over the arrays DIMmed together.
struct Groups {
    parent: BTreeMap<Identity, Identity>,
    bytes: BTreeMap<Identity, i64>,
}

impl Groups {
    fn root(&self, mut one: Identity) -> Identity {
        while self.parent[&one] != one {
            one = self.parent[&one];
        }
        one
    }
}

/// Groups of arrays worth one allocation: DIMmed in the same block, all
/// global or all local, read in the same blocks, together at most 64K. Each
/// group in DIM order.
fn grouped(survey: &Survey) -> Vec<Vec<Identity>> {
    let members = &survey.members;
    let mut affinity: BTreeMap<(Identity, Identity), usize> = BTreeMap::new();
    for block in &survey.accessed {
        let here: Vec<Identity> = block.iter().copied().filter(|one| members.contains_key(one)).collect();
        for (index, one) in here.iter().enumerate() {
            for other in &here[index + 1..] {
                // One DIM site, and one kind of descriptor place to retarget.
                let site = |identity: &Identity| {
                    (members[identity].function, members[identity].block, matches!(identity, Identity::Global(..)))
                };
                if site(one) == site(other) {
                    *affinity.entry((*one, *other)).or_default() += 1;
                }
            }
        }
    }
    let mut pairs: Vec<((Identity, Identity), usize)> = affinity.into_iter().collect();
    pairs.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));
    let mut groups = Groups {
        parent: members.keys().map(|one| (*one, *one)).collect(),
        bytes: members.iter().map(|(one, member)| (*one, member.bytes() + member.align() - 1)).collect(),
    };
    for ((one, other), _) in pairs {
        let (one, other) = (groups.root(one), groups.root(other));
        let total = groups.bytes[&one] + groups.bytes[&other];
        if one != other && total <= SEGMENT {
            groups.parent.insert(other, one);
            groups.bytes.insert(one, total);
        }
    }
    let mut out: BTreeMap<Identity, Vec<Identity>> = BTreeMap::new();
    for one in members.keys() {
        out.entry(groups.root(*one)).or_default().push(*one);
    }
    out.into_values()
        .filter(|group| group.len() > 1)
        // A release frees the whole allocation: every member goes at once.
        .filter(|group| {
            survey.released.iter().all(|freed| {
                let here = group.iter().filter(|one| freed.contains(one)).count();
                here == 0 || here == group.len()
            })
        })
        .map(|mut group| {
            group.sort_by_key(|one| (members[one].function, members[one].block, members[one].position));
            group
        })
        .collect()
}

/// A fresh value of `type_id` in `function`: ids are numbered per function.
fn fresh(function: &mut Function, type_id: u32) -> u32 {
    let id = function.values.iter().map(|(id, _)| id + 1).max().unwrap_or(1);
    function.values.push((id, type_id));
    id
}

/// Where a member's elements are: its first element's byte in the group,
/// and the bytes from one element to the next.
#[derive(Clone, Copy)]
struct Layout {
    at: i64,
    stride: i64,
}

/// Each member's layout and the group's size. With `interleaved`, same-shape
/// members some block reads at one subscript share records; the rest, and
/// all without it, follow one another.
fn laid_out(group: &[Identity], survey: &Survey, interleaved: bool) -> (BTreeMap<Identity, Layout>, i64) {
    let members = &survey.members;
    let joins = |one: &Identity, other: &Identity| {
        members[one].records == members[other].records
            && !survey.unstrided.contains(one)
            && !survey.unstrided.contains(other)
            && survey.walked.contains(&(*one.min(other), *one.max(other)))
    };
    let mut records: Vec<Vec<Identity>> = Vec::new();
    for one in group {
        match records.iter_mut().find(|record| interleaved && record.iter().all(|other| joins(one, other))) {
            Some(record) => record.push(*one),
            None => records.push(vec![*one]),
        }
    }
    let (mut layouts, mut end) = (BTreeMap::new(), 0);
    for record in records {
        let align = record.iter().map(|one| members[one].align()).max().expect("a member");
        let mut fields = Vec::new();
        let mut size = 0;
        for one in &record {
            let at = rounded(size, members[one].align());
            fields.push(at);
            size = at + members[one].width;
        }
        let stride = if record.len() == 1 { size } else { rounded(size, align) };
        let at = rounded(end, align);
        for (one, field) in record.iter().zip(fields) {
            layouts.insert(*one, Layout { at: at + field, stride });
        }
        end = at + members[&record[0]].count() * stride;
    }
    (layouts, end)
}

/// Lay `group` out in one allocation, the first member's descriptor its own.
fn merged(compiler: &mut Compiler, group: &[Identity], survey: &Survey) {
    let members = &survey.members;
    let (mut offsets, mut end) = laid_out(group, survey, true);
    // Record padding can push a group past 64K; end to end always fits.
    if end > SEGMENT {
        (offsets, end) = laid_out(group, survey, false);
    }
    let words = (end + 1) / 2;
    let first = group[0];
    // Where the first member's descriptor is: its data symbol and offset,
    // or its frame place.
    let (location, local) = {
        let function = &compiler.functions[members[&first].function];
        let pointers = shapes::pointers(function);
        let pointer = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find_map(|one| match &one.tag {
                Some(Tag::Allocate(shape)) if pointers.get(&shape.descriptor) == Some(&first) => Some(shape.descriptor),
                _ => None,
            })
            .expect("a member's DIM");
        let place = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find(|one| one.op == "address" && one.results == [pointer])
            .and_then(|one| match one.operands.as_slice() {
                [Operand::Place(place)] => function.places.iter().find(|candidate| candidate.id == *place),
                _ => None,
            })
            .expect("a descriptor pointer is its place's address");
        ((place.symbol, place.offset), place.id)
    };
    for index in 0..compiler.functions.len() {
        let pointers = shapes::pointers(&compiler.functions[index]);
        let member_of = |value: &u32| pointers.get(value).filter(|one| offsets.contains_key(one)).copied();
        // Every member's descriptor pointer names the first member's descriptor.
        let function = &mut compiler.functions[index];
        let identities: BTreeMap<u32, Identity> =
            function.places.iter().map(|place| (place.id, shapes::identity(function, place))).collect();
        for place in &mut function.places {
            if identities[&place.id] != first && offsets.contains_key(&identities[&place.id]) {
                if let Identity::Global(..) = identities[&place.id] {
                    (place.symbol, place.offset) = location;
                }
            }
        }
        for block in &mut function.blocks {
            for one in &mut block.instructions {
                if let ("address", [Operand::Place(place)]) = (one.op, one.operands.as_slice()) {
                    if matches!(identities[place], Identity::Local(..)) && identities[place] != first && offsets.contains_key(&identities[place]) {
                        one.operands = vec![Operand::Place(local)];
                    }
                }
            }
        }
        let types: BTreeMap<u32, u32> = function.values.iter().copied().collect();
        let mut removed: BTreeSet<u32> = BTreeSet::new();
        let mut reordered: BTreeSet<u32> = BTreeSet::new();
        let mut added: Vec<(usize, usize, u32, i64, i64)> = Vec::new();
        for (at, block) in compiler.functions[index].blocks.iter_mut().enumerate() {
            for (position, one) in block.instructions.iter_mut().enumerate() {
                match one.tag.take() {
                    Some(Tag::Allocate(shape)) if member_of(&shape.descriptor) == Some(first) => {
                        // The whole group, as words.
                        let flags = constant(&one.operands[one.operands.len() - 2]).expect("B$DDIM's flags are constant");
                        one.operands = vec![
                            Operand::Constant(INTEGER, Number::Integer(0)),
                            Operand::Constant(INTEGER, Number::Integer(words - 1)),
                            Operand::Constant(INTEGER, Number::Integer(2)),
                            Operand::Constant(INTEGER, Number::Integer(flags & !0xFF | 1)),
                            Operand::Value(shape.descriptor),
                        ];
                        let records = vec![(Operand::Constant(INTEGER, Number::Integer(0)), Operand::Constant(INTEGER, Number::Integer(words - 1)))];
                        one.tag = Some(Tag::Allocate(Shape { descriptor: shape.descriptor, records, element: INTEGER }));
                        reordered.insert(one.id);
                    }
                    Some(Tag::Allocate(shape)) if member_of(&shape.descriptor).is_some() => {
                        removed.insert(one.id);
                    }
                    Some(Tag::Release { descriptor }) if member_of(&descriptor).is_some_and(|one| one != first) => {
                        removed.insert(one.id);
                    }
                    Some(Tag::DescriptorField { descriptor, field }) if member_of(&descriptor).is_some() => {
                        let member = &members[&member_of(&descriptor).expect("a member")];
                        let fixed = match field {
                            Slot::Count(record) => Some(member.records[record].1 - member.records[record].0 + 1),
                            Slot::Lower(record) => Some(member.records[record].0),
                            Slot::Rank => Some(member.records.len() as i64),
                            _ => None,
                        };
                        match fixed {
                            Some(fixed) => {
                                one.op = "copy";
                                one.operands = vec![Operand::Constant(types[&one.results[0]], Number::Integer(fixed))];
                            }
                            None => one.tag = Some(Tag::DescriptorField { descriptor, field }),
                        }
                    }
                    Some(Tag::ElementOffset { descriptor, origin }) if member_of(&descriptor).is_some() => {
                        let identity = member_of(&descriptor).expect("a member");
                        let Layout { at: start, stride } = offsets[&identity];
                        let shift = start - members[&identity].adjustment() * stride;
                        // 16-bit offsets wrap: the same sum as a signed word.
                        let shift = (shift + 0x8000).rem_euclid(SEGMENT) - 0x8000;
                        if shift != 0 || stride != members[&identity].width {
                            added.push((at, position, one.id, shift, stride));
                        }
                        one.tag = Some(Tag::ElementOffset { descriptor, origin });
                    }
                    other => one.tag = other,
                }
            }
        }
        // `origin + subscript * width` becomes
        // `origin + (subscript * stride + shift)`.
        let function: &mut Function = &mut compiler.functions[index];
        for (at, position, id, shift, stride) in added.into_iter().rev() {
            if let Operand::Value(bytes) = function.blocks[at].instructions[position].operands[1] {
                let scaled = function.blocks[at].instructions.iter_mut().find(|one| one.results == [bytes]);
                if let Some(Operand::Constant(_, Number::Integer(width))) = scaled.and_then(|one| one.operands.get_mut(1)) {
                    *width = stride;
                }
            }
            if shift == 0 {
                continue;
            }
            // The shift is added last, where it can become a displacement;
            // the element's offset is then the new sum.
            let shifted = fresh(function, INTEGER);
            let instruction = function.blocks.iter().flat_map(|block| &block.instructions).map(|one| one.id + 1).max().unwrap_or(1);
            let block = &mut function.blocks[at];
            debug_assert_eq!(block.instructions[position].id, id);
            let sum = block.instructions[position].results[0];
            let tag = block.instructions[position].tag.take();
            for one in block.instructions.iter_mut().skip(position + 1) {
                for operand in &mut one.operands {
                    if matches!(operand, Operand::Value(value) if *value == sum) {
                        *operand = Operand::Value(shifted);
                    }
                }
            }
            block.instructions.insert(position + 1, Instruction {
                id: instruction,
                op: "add",
                results: vec![shifted],
                operands: vec![Operand::Value(sum), Operand::Constant(INTEGER, Number::Integer(shift))],
                callee: None,
                tag,
                nowrap: false,
            });
        }
        for block in &mut function.blocks {
            block.instructions.retain(|one| !removed.contains(&one.id));
        }
        function.calls.retain(|call| !removed.contains(&call.instruction));
        for call in function.calls.iter_mut().filter(|call| reordered.contains(&call.instruction)) {
            call.order = (0..5).collect();
        }
    }
}

/// Merge every group the module allows.
pub(super) fn applied(compiler: &mut Compiler) {
    let survey = surveyed(compiler);
    for group in grouped(&survey) {
        merged(compiler, &group, &survey);
    }
}

#[cfg(test)]
mod tests {
    use super::super::{built, Options};
    use super::*;
    use crate::{parse, Dialect};

    fn applied_to(source: &str) -> Compiler {
        let module = parse(source, Dialect::VbDos).expect("parses");
        let mut compiler = built(&module, "T", Dialect::VbDos, "vbdos", &Options::default())
            .unwrap_or_else(|error| panic!("{}", error.message));
        applied(&mut compiler);
        compiler
    }

    fn instructions(compiler: &Compiler) -> Vec<&Instruction> {
        compiler.functions.iter().flat_map(|function| &function.blocks).flat_map(|block| &block.instructions).collect()
    }

    /// Every DIM's records, as constants.
    fn allocations(compiler: &Compiler) -> Vec<Vec<(i64, i64)>> {
        instructions(compiler)
            .iter()
            .filter_map(|one| match &one.tag {
                Some(Tag::Allocate(shape)) => {
                    Some(shape.records.iter().map(|(lower, upper)| (constant(lower).unwrap(), constant(upper).unwrap())).collect())
                }
                _ => None,
            })
            .collect()
    }

    /// The constant each element offset is shifted by.
    fn shifts(compiler: &Compiler) -> Vec<i64> {
        instructions(compiler)
            .iter()
            .filter(|one| matches!(one.tag, Some(Tag::ElementOffset { .. })))
            .map(|one| constant(&one.operands[1]).unwrap_or(0))
            .collect()
    }

    /// The bytes between elements each element offset steps by.
    fn strides(compiler: &Compiler) -> Vec<i64> {
        let all = instructions(compiler);
        all.iter()
            .filter(|one| matches!(one.tag, Some(Tag::ElementOffset { .. })))
            .map(|one| {
                // Past the shift, if any, to `origin + bytes`.
                let sum = match &one.operands[..] {
                    [Operand::Value(sum), Operand::Constant(..)] => all.iter().find(|other| other.results == [*sum]).expect("defined"),
                    _ => one,
                };
                let mut value = sum.operands[1].clone();
                loop {
                    let Operand::Value(id) = value else { panic!("an element offset adds a value") };
                    let defining = all.iter().find(|other| other.results == [id]).expect("defined");
                    match (defining.op, defining.operands.as_slice()) {
                        ("mul", [_, Operand::Constant(_, Number::Integer(stride))]) => break *stride,
                        _ => value = defining.operands[0].clone(),
                    }
                }
            })
            .collect()
    }

    #[test]
    fn arrays_read_at_one_subscript_become_one_array_of_records() {
        let compiler = applied_to("'$DYNAMIC\nDIM a(10) AS INTEGER, b(10) AS INTEGER\nFOR i = 1 TO 5\nb(i) = a(i)\nNEXT\n");
        // Records of a then b, 4 bytes each: 11 records, 22 words.
        assert_eq!(allocations(&compiler), vec![vec![(0, 21)]]);
        assert_eq!(strides(&compiler), vec![4, 4]);
        assert_eq!(shifts(&compiler), vec![2, 0]);
    }

    /// `xo%(60) = 5` kept deedlines' xo%, yo%, zo% end to end.
    #[test]
    fn a_constant_subscript_still_steps_by_the_record() {
        let compiler = applied_to("'$DYNAMIC\nDIM a(10) AS INTEGER, b(10) AS INTEGER\nFOR i = 1 TO 5\nb(i) = a(i)\nNEXT\na(3) = 1\n");
        assert_eq!(strides(&compiler), vec![4, 4, 4]);
    }

    /// An element's far address handed on would let the callee walk the
    /// array at a stride the records no longer have.
    #[test]
    fn an_element_address_passed_on_keeps_its_array_stride() {
        let compiler = applied_to(
            "DECLARE SUB s (SEG x AS INTEGER)\n'$DYNAMIC\nDIM a(10) AS INTEGER, b(10) AS INTEGER\nFOR i = 1 TO 5\nb(i) = a(i)\nNEXT\nCALL s(a(0))\n",
        );
        assert_eq!(allocations(&compiler), vec![vec![(0, 21)]]);
        assert_eq!(strides(&compiler), vec![2, 2, 2]);
    }

    #[test]
    fn arrays_read_at_different_subscripts_follow_one_another() {
        let compiler = applied_to("'$DYNAMIC\nDIM a(10) AS INTEGER, b(10) AS INTEGER\nFOR i = 1 TO 5\nb(i) = a(i + 1)\nNEXT\n");
        assert_eq!(allocations(&compiler), vec![vec![(0, 21)]]);
        assert_eq!(strides(&compiler), vec![2, 2]);
        assert_eq!(shifts(&compiler), vec![22, 0]);
    }

    const READ_TOGETHER: &str = "'$DYNAMIC\nDIM a(10) AS INTEGER, b(1 TO 5) AS LONG\nFOR i = 1 TO 5\nb(i) = a(i)\nNEXT\n";

    #[test]
    fn arrays_read_together_share_one_allocation() {
        let compiler = applied_to(READ_TOGETHER);
        // a: 22 bytes at 0; b: 20 bytes at 24, its element 1 at 24.
        assert_eq!(allocations(&compiler), vec![vec![(0, 21)]]);
        assert_eq!(shifts(&compiler), vec![24 - 4, 0]);
    }

    /// Value and instruction ids were drawn from the compiler's counter,
    /// which restarts per function: a merged SUB's HIR was rejected.
    #[test]
    fn merging_in_a_sub_keeps_its_ids_unique() {
        let compiler = applied_to(&format!("DECLARE SUB s ()\nPRINT\nSUB s\n{READ_TOGETHER}END SUB\n"));
        assert_eq!(allocations(&compiler), vec![vec![(0, 21)]]);
        let sub = compiler.functions.iter().find(|one| one.name.eq_ignore_ascii_case("s")).expect("s");
        let values: BTreeSet<u32> = sub.values.iter().map(|(id, _)| *id).collect();
        let instructions: BTreeSet<u32> = sub.blocks.iter().flat_map(|block| &block.instructions).map(|one| one.id).collect();
        assert_eq!(values.len(), sub.values.len());
        assert_eq!(instructions.len(), sub.blocks.iter().map(|block| block.instructions.len()).sum::<usize>());
    }

    #[test]
    fn a_whole_array_argument_keeps_its_own_allocation() {
        let compiler = applied_to(&format!("DECLARE SUB s (x() AS INTEGER)\n{READ_TOGETHER}CALL s(a())\nSUB s (x() AS INTEGER)\nEND SUB\n"));
        assert_eq!(allocations(&compiler).len(), 2);
    }

    #[test]
    fn a_redimmed_array_keeps_its_own_allocation() {
        let compiler = applied_to(&format!("{READ_TOGETHER}REDIM a(20) AS INTEGER\n"));
        assert_eq!(allocations(&compiler).len(), 2);
    }

    #[test]
    fn arrays_never_read_together_stay_apart() {
        let compiler = applied_to("'$DYNAMIC\nDIM a(10) AS INTEGER, b(10) AS INTEGER\nFOR i = 1 TO 5\na(i) = 1\nNEXT\nFOR i = 1 TO 5\nb(i) = 2\nNEXT\n");
        assert_eq!(allocations(&compiler).len(), 2);
    }

    #[test]
    fn arrays_over_64k_together_stay_apart() {
        let compiler = applied_to("'$DYNAMIC\nDIM a(20000) AS INTEGER, b(20000) AS INTEGER\nb(1) = a(1)\n");
        assert_eq!(allocations(&compiler).len(), 2);
    }

    #[test]
    fn erasing_one_member_keeps_them_apart() {
        let compiler = applied_to(&format!("{READ_TOGETHER}ERASE a\n"));
        assert_eq!(allocations(&compiler).len(), 2);
    }
}
