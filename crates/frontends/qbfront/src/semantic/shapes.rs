//! Array shapes over the whole module.
//!
//! Every DIM, REDIM and static array's fixed bounds that can reach a
//! descriptor is known here: its own procedure's, and through whole-array
//! arguments, its callers' and callees'.
//! Where all of them agree, the shape is a fact: the rank and constant
//! dimension counts and lower bounds replace their descriptor reads, and a zero-based array's descriptor offset
//! is its first byte, which each element access records as its origin.
//!
//! B$DDIM puts a far array's data at offset 0 of its own segment and never
//! moves it within that segment, so where every allocation is far with
//! constant bounds, the adjusted offset at +0Ah is a constant too.
//!
//! A descriptor another module can reach -- an external place, or an array
//! parameter of an externally callable procedure -- has no fact, nor has one
//! handed to a callee this module does not define.

use std::collections::{BTreeMap, BTreeSet};

use super::tags::{Passing, Slot, Tag};
use super::{Compiler, Function, Instruction, Number, Operand, Place};

/// A descriptor, independently of the value that points to it.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Identity {
    /// A module or STATIC descriptor: its data symbol and offset.
    Global(u32, isize),
    /// A procedure's local descriptor.
    Local(u32, u32),
    /// A procedure's array parameter.
    Parameter(u32, usize),
}

/// What every allocation of one descriptor agrees on.
#[derive(Clone, Debug, Default)]
struct Known {
    unknown: bool,
    counts: Option<Vec<i64>>,
    lowers: Option<Vec<i64>>,
    rank: Option<usize>,
    origin: Option<i64>,
    allocations: usize,
}

impl Known {
    /// Fold in one allocation's per-record bounds and, for a far one, its
    /// adjusted offset.
    fn merged(&mut self, records: &[(Operand, Operand)], origin: Option<i64>) {
        let constant = |operand: &Operand| match operand {
            Operand::Constant(_, Number::Integer(value)) => Some(*value),
            _ => None,
        };
        let counts: Option<Vec<i64>> = records
            .iter()
            .map(|(lower, upper)| Some(constant(upper)? - constant(lower)? + 1))
            .collect();
        let lowers: Option<Vec<i64>> = records.iter().map(|(lower, _)| constant(lower)).collect();
        if self.allocations == 0 {
            (self.counts, self.lowers, self.rank, self.origin) = (counts, lowers, Some(records.len()), origin);
        } else {
            if self.rank != Some(records.len()) {
                self.rank = None;
            }
            if self.counts != counts {
                self.counts = None;
            }
            if self.lowers != lowers {
                self.lowers = None;
            }
            if self.origin != origin {
                self.origin = None;
            }
        }
        self.allocations += 1;
    }

    /// Whether every allocation's lower bounds are zero.
    fn zero_based(&self) -> bool {
        self.lowers.as_ref().is_some_and(|lowers| lowers.iter().all(|&lower| lower == 0))
    }

    /// Whether anything here is a fact.
    fn proven(&self) -> bool {
        !self.unknown && self.allocations != 0
    }
}

/// The adjusted offset B$DDIM or B$RDIM leaves at +0Ah for these arguments,
/// where the allocation is far: its data starts at offset 0.
///
/// The runtime runs the records in order, `adjustment * count - lower`, then
/// scales by the element size and keeps the low word.
fn far_origin(operands: &[Operand], records: &[(Operand, Operand)]) -> Option<i64> {
    const FAR: i64 = 1;
    let constant = |operand: &Operand| match operand {
        Operand::Constant(_, Number::Integer(value)) => Some(*value),
        _ => None,
    };
    let [.., size, flags, _descriptor] = operands else {
        return None;
    };
    if (constant(flags)? >> 8) & 3 != FAR {
        return None;
    }
    let mut adjustment = 0_i64;
    for (lower, upper) in records {
        let (lower, upper) = (constant(lower)?, constant(upper)?);
        adjustment = adjustment * (upper - lower + 1) - lower;
    }
    Some(((adjustment * constant(size)?) as i16).into())
}

/// Union-find over descriptor identities.
#[derive(Default)]
struct Classes {
    parent: BTreeMap<Identity, Identity>,
}

impl Classes {
    fn root(&mut self, one: Identity) -> Identity {
        let parent = *self.parent.entry(one).or_insert(one);
        if parent == one {
            return one;
        }
        let root = self.root(parent);
        self.parent.insert(one, root);
        root
    }

    fn join(&mut self, one: Identity, other: Identity) {
        let (one, other) = (self.root(one), self.root(other));
        if one != other {
            self.parent.insert(one, other);
        }
    }
}

/// The descriptor `place` of `function` is.
pub(super) fn identity(function: &Function, place: &Place) -> Identity {
    match place.storage {
        "local" => Identity::Local(function.id, place.id),
        _ => Identity::Global(place.symbol, place.offset),
    }
}

/// Each descriptor pointer value of `function` and the descriptor it names.
pub(super) fn pointers(function: &Function) -> BTreeMap<u32, Identity> {
    let mut out: BTreeMap<u32, Identity> = function
        .parameters
        .iter()
        .enumerate()
        .map(|(index, value)| (*value, Identity::Parameter(function.id, index)))
        .collect();
    for one in function.blocks.iter().flat_map(|block| &block.instructions) {
        let (true, [result], [Operand::Place(place)]) =
            (one.op == "address", one.results.as_slice(), one.operands.as_slice())
        else {
            continue;
        };
        let Some(place) = function.places.iter().find(|candidate| candidate.id == *place) else {
            continue;
        };
        out.insert(*result, identity(function, place));
    }
    out
}

/// The shapes every descriptor's allocations agree on, by class root.
fn known(compiler: &Compiler) -> (Classes, BTreeMap<Identity, Known>) {
    let defined: BTreeMap<u32, &Function> = compiler
        .functions
        .iter()
        .filter_map(|function| {
            let symbol = compiler.signatures.get(super::canonical(&function.name))?.symbol;
            Some((symbol, function))
        })
        .collect();
    let mut classes = Classes::default();
    let mut unknown: Vec<Identity> = Vec::new();
    let statics: BTreeMap<Identity, &[(Operand, Operand)]> = compiler
        .functions
        .iter()
        .flat_map(|function| function.places.iter().map(move |place| (function, place)))
        .filter_map(|(function, place)| Some((identity(function, place), compiler.static_shapes.get(&place.id)?.as_slice())))
        .collect();
    let mut shapes: Vec<(Identity, &[(Operand, Operand)], Option<i64>)> =
        statics.into_iter().map(|(identity, records)| (identity, records, None)).collect();
    let mut handed: Vec<Identity> = Vec::new();
    for function in &compiler.functions {
        let pointers = pointers(function);
        for place in &function.places {
            if place.storage == "external" {
                unknown.push(Identity::Global(place.symbol, place.offset));
            }
        }
        if function.linkage == "external" {
            unknown.extend((0..function.parameters.len()).map(|index| Identity::Parameter(function.id, index)));
        }
        for one in function.blocks.iter().flat_map(|block| &block.instructions) {
            match &one.tag {
                Some(Tag::Allocate(shape) | Tag::Reallocate(shape)) => match pointers.get(&shape.descriptor) {
                    Some(identity) => shapes.push((*identity, &shape.records, far_origin(&one.operands, &shape.records))),
                    // An allocation of a descriptor with no identity could be any.
                    None => return (classes, BTreeMap::new()),
                },
                Some(Tag::Invoke { arguments }) => {
                    let callee = function
                        .calls
                        .iter()
                        .find(|call| call.instruction == one.id)
                        .and_then(|call| call.callee)
                        .and_then(|symbol| defined.get(&symbol));
                    for (index, passing) in arguments.iter().enumerate() {
                        let Passing::Array(value) = passing else {
                            continue;
                        };
                        let Some(identity) = pointers.get(value) else {
                            return (classes, BTreeMap::new());
                        };
                        handed.push(*identity);
                        match callee {
                            Some(callee) => classes.join(*identity, Identity::Parameter(callee.id, index)),
                            None => unknown.push(*identity),
                        }
                    }
                }
                _ => {
                    // A descriptor pointer stored or copied escapes this analysis.
                    let escaped = matches!(one.op, "store" | "copy")
                        && one.operands.iter().any(|operand| {
                            matches!(operand, Operand::Value(value) if pointers.contains_key(value))
                        });
                    if escaped {
                        for operand in &one.operands {
                            if let Operand::Value(value) = operand {
                                unknown.extend(pointers.get(value));
                            }
                        }
                    }
                }
            }
        }
    }
    // A descriptor handed on with no DIM, REDIM or static bounds here has
    // no known shape.
    let allocated: Vec<Identity> = shapes.iter().map(|(identity, ..)| *identity).collect();
    unknown.extend(handed.into_iter().filter(|identity| {
        !matches!(identity, Identity::Parameter(..)) && !allocated.contains(identity)
    }));
    let mut out: BTreeMap<Identity, Known> = BTreeMap::new();
    for (identity, records, origin) in shapes {
        let root = classes.root(identity);
        out.entry(root).or_default().merged(records, origin);
    }
    for identity in unknown {
        let root = classes.root(identity);
        out.entry(root).or_default().unknown = true;
    }
    (classes, out)
}

/// A function's entry block.
const ENTRY: u32 = 1;

/// The descriptors a function holds allocated before each instruction that
/// asks, walking its blocks to a fixed point: a DIM or REDIM allocates, ERASE
/// releases, and a call of a user procedure releases whatever it can reach --
/// the `shared` module arrays, parameters and arrays handed whole. Every external entry
/// starts with none. `entry` is what the function's own entry holds.
fn allocated_before(
    function: &Function,
    pointers: &BTreeMap<u32, Identity>,
    entry: &BTreeSet<Identity>,
    statics: &BTreeSet<Identity>,
    shared: &BTreeSet<Identity>,
) -> BTreeMap<u32, BTreeSet<Identity>> {
    let transfer = |one: &Instruction, state: &mut BTreeSet<Identity>| match &one.tag {
        Some(Tag::Allocate(shape) | Tag::Reallocate(shape)) => state.extend(pointers.get(&shape.descriptor)),
        Some(Tag::Release { descriptor }) => {
            if let Some(identity) = pointers.get(descriptor) {
                state.remove(identity);
            }
        }
        Some(Tag::Invoke { arguments }) => {
            state.retain(|identity| matches!(identity, Identity::Local(..)) || (matches!(identity, Identity::Global(..)) && !shared.contains(identity)));
            for passing in arguments {
                if let Passing::Array(value) = passing {
                    state.remove(pointers.get(value).unwrap_or(&Identity::Global(u32::MAX, 0)));
                }
            }
        }
        _ => {}
    };
    let mut predecessors: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for block in &function.blocks {
        for target in block.terminator.iter().flat_map(|one| &one.targets) {
            predecessors.entry(*target).or_default().push(block.id);
        }
    }
    // None is "every descriptor": a block no walk has reached yet.
    let mut out: BTreeMap<u32, Option<BTreeSet<Identity>>> = function.blocks.iter().map(|block| (block.id, None)).collect();
    let before = |block: &super::Block, out: &BTreeMap<u32, Option<BTreeSet<Identity>>>| -> BTreeSet<Identity> {
        if block.id == ENTRY {
            return entry.clone();
        }
        if function.external_entries.contains(&block.id) {
            return BTreeSet::new();
        }
        let mut joined: Option<BTreeSet<Identity>> = None;
        for from in predecessors.get(&block.id).into_iter().flatten() {
            if let Some(state) = &out[from] {
                joined = Some(match joined {
                    None => state.clone(),
                    Some(joined) => joined.intersection(state).copied().collect(),
                });
            }
        }
        joined.unwrap_or_default()
    };
    loop {
        let mut changed = false;
        for block in &function.blocks {
            let reached = block.id == ENTRY
                || function.external_entries.contains(&block.id)
                || predecessors.get(&block.id).into_iter().flatten().any(|from| out[from].is_some());
            if !reached {
                continue;
            }
            let mut state = before(block, &out);
            block.instructions.iter().for_each(|one| transfer(one, &mut state));
            if out[&block.id].as_ref() != Some(&state) {
                out.insert(block.id, Some(state));
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut found = BTreeMap::new();
    for block in &function.blocks {
        let mut state = before(block, &out);
        for one in &block.instructions {
            if matches!(one.tag, Some(Tag::Allocated { .. } | Tag::Invoke { .. })) {
                let mut seen = state.clone();
                seen.extend(statics.iter().copied());
                found.insert(one.id, seen);
            }
            transfer(one, &mut state);
        }
    }
    found
}

/// The class roots of array parameters that every call hands an allocated
/// array: optimistically all, then each class one call does not.
fn entered_allocated(
    compiler: &Compiler,
    classes: &mut Classes,
    known: &BTreeMap<Identity, Known>,
    statics: &BTreeSet<Identity>,
    shared: &BTreeSet<Identity>,
) -> BTreeSet<Identity> {
    let defined: BTreeSet<u32> = compiler
        .functions
        .iter()
        .filter_map(|function| compiler.signatures.get(super::canonical(&function.name)).map(|signature| signature.symbol))
        .collect();
    let mut ok: BTreeSet<Identity> = BTreeSet::new();
    for function in &compiler.functions {
        for index in 0..function.parameters.len() {
            let root = classes.root(Identity::Parameter(function.id, index));
            if known.get(&root).is_some_and(Known::proven) {
                ok.insert(root);
            }
        }
    }
    loop {
        let before = ok.len();
        for function in &compiler.functions {
            let pointers = pointers(function);
            let mut entry = BTreeSet::new();
            for index in 0..function.parameters.len() {
                let identity = Identity::Parameter(function.id, index);
                if ok.contains(&classes.root(identity)) {
                    entry.insert(identity);
                }
            }
            let states = allocated_before(function, &pointers, &entry, statics, shared);
            for one in function.blocks.iter().flat_map(|block| &block.instructions) {
                let Some(Tag::Invoke { arguments }) = &one.tag else { continue };
                let reaches = function.calls.iter().find(|call| call.instruction == one.id).and_then(|call| call.callee).is_some_and(|symbol| defined.contains(&symbol));
                if !reaches {
                    continue;
                }
                for passing in arguments {
                    let Passing::Array(value) = passing else { continue };
                    let Some(identity) = pointers.get(value) else { continue };
                    if !states[&one.id].contains(identity) {
                        ok.remove(&classes.root(*identity));
                    }
                }
            }
        }
        if ok.len() == before {
            return ok;
        }
    }
}

/// Fold each proven fact into the HIR.
pub(super) fn applied(compiler: &mut Compiler) {
    let (mut classes, known) = known(compiler);
    let statics: BTreeSet<Identity> = compiler
        .functions
        .iter()
        .flat_map(|function| function.places.iter().map(move |place| (function, place)))
        .filter(|(_, place)| compiler.static_shapes.contains_key(&place.id))
        .map(|(function, place)| identity(function, place))
        .collect();
    // A module array two procedures name is one a call may ERASE.
    let mut named: BTreeMap<Identity, BTreeSet<u32>> = BTreeMap::new();
    for function in &compiler.functions {
        for one in pointers(function).into_values().filter(|one| matches!(one, Identity::Global(..))) {
            named.entry(one).or_default().insert(function.id);
        }
    }
    let shared: BTreeSet<Identity> = named.into_iter().filter(|(_, users)| users.len() > 1).map(|(one, _)| one).collect();
    let entered = entered_allocated(compiler, &mut classes, &known, &statics, &shared);
    for function in &mut compiler.functions {
        let pointers = pointers(function);
        let entry: BTreeSet<Identity> = (0..function.parameters.len())
            .map(|index| Identity::Parameter(function.id, index))
            .filter(|identity| entered.contains(&classes.root(*identity)))
            .collect();
        let held = allocated_before(function, &pointers, &entry, &statics, &shared);
        let mut fact = |descriptor: u32| {
            let identity = pointers.get(&descriptor)?;
            known.get(&classes.root(*identity)).filter(|one| one.proven()).cloned()
        };
        let types: BTreeMap<u32, u32> = function.values.iter().copied().collect();
        // A descriptor this function names as a place: a parameter's has none.
        let places: BTreeMap<Identity, u32> =
            function.places.iter().rev().map(|place| (identity(function, place), place.id)).collect();
        let owner = |descriptor: u32| pointers.get(&descriptor).and_then(|identity| places.get(identity)).copied();

        for block in &mut function.blocks {
            for one in &mut block.instructions {
                match one.tag {
                    Some(Tag::DescriptorField {
                        descriptor,
                        field: field @ (Slot::Count(_) | Slot::Lower(_) | Slot::CountOf(_) | Slot::LowerOf(_)),
                    }) => {
                        let Some(fact) = fact(descriptor) else { continue };
                        let record = match field {
                            Slot::Count(record) | Slot::Lower(record) => Some(record),
                            Slot::CountOf(dimension) | Slot::LowerOf(dimension) => fact.rank.and_then(|rank| rank.checked_sub(dimension)),
                            _ => None,
                        };
                        let bounds = if matches!(field, Slot::Count(_) | Slot::CountOf(_)) { fact.counts } else { fact.lowers };
                        let Some(&bound) = bounds.as_ref().zip(record).and_then(|(bounds, record)| bounds.get(record)) else {
                            continue;
                        };
                        one.op = "copy";
                        one.operands = vec![Operand::Constant(types[&one.results[0]], Number::Integer(bound))];
                    }
                    Some(Tag::Allocated { descriptor }) if one.op == "ne" => {
                        if !pointers.get(&descriptor).is_some_and(|identity| held[&one.id].contains(identity)) {
                            continue;
                        }
                        one.op = "copy";
                        one.operands = vec![Operand::Constant(types[&one.results[0]], Number::Integer(1))];
                    }
                    Some(Tag::DescriptorField { descriptor, field: Slot::Rank }) => {
                        let Some(rank) = fact(descriptor).and_then(|fact| fact.rank) else {
                            continue;
                        };
                        one.op = "copy";
                        one.operands = vec![Operand::Constant(types[&one.results[0]], Number::Integer(rank as i64))];
                    }
                    Some(Tag::DescriptorField { descriptor, field: Slot::Origin }) if one.op == "load" => {
                        let Some(origin) = fact(descriptor).and_then(|fact| fact.origin) else {
                            continue;
                        };
                        one.op = "copy";
                        one.operands = vec![Operand::Constant(types[&one.results[0]], Number::Integer(origin))];
                    }
                    Some(Tag::ElementOffset { descriptor, origin }) => {
                        if let Some(origin) = origin.filter(|_| fact(descriptor).is_some_and(|fact| fact.zero_based())) {
                            function.origins.extend(one.results.iter().map(|result| (*result, origin)));
                        }
                        // An element is in its own array's allocation.
                        if let Some(place) = owner(descriptor) {
                            function.allocations.extend(one.results.iter().map(|result| (*result, place)));
                        }
                    }
                    _ => {}
                }
            }
        }
        // A split far offset reaches memory as the low half of a CONCAT.
        let mut concatenated = Vec::new();
        for one in function.blocks.iter().flat_map(|block| &block.instructions) {
            if let ("concat", [result], [_, Operand::Value(offset)]) =
                (one.op, one.results.as_slice(), one.operands.as_slice())
            {
                concatenated.push((*result, *offset));
            }
        }
        for (result, offset) in concatenated {
            if let Some(origin) = function.origins.get(&offset).copied() {
                function.origins.insert(result, origin);
            }
            if let Some(place) = function.allocations.get(&offset).copied() {
                function.allocations.insert(result, place);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{built, Compiler, Instruction, Options};
    use super::*;
    use crate::{parse, Dialect};

    fn applied_to(source: &str) -> Compiler {
        applied_in(source, false)
    }

    fn applied_in(source: &str, row_major: bool) -> Compiler {
        applied_with(source, &Options { row_major, ..Options::default() })
    }

    fn applied_with(source: &str, options: &Options) -> Compiler {
        let module = parse(source, Dialect::VbDos).expect("parses");
        let mut compiler = built(&module, "T", Dialect::VbDos, "vbdos", options)
            .unwrap_or_else(|error| panic!("{}", error.message));
        applied(&mut compiler);
        compiler
    }

    fn function<'a>(compiler: &'a Compiler, name: &str) -> &'a Function {
        compiler.functions.iter().find(|one| one.name.eq_ignore_ascii_case(name)).expect(name)
    }

    /// The constants each count read became.
    fn counts(function: &Function) -> Vec<i64> {
        function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter(|one| matches!(one.tag, Some(Tag::DescriptorField { field: Slot::Count(_), .. })))
            .filter_map(|one| match (one.op, one.operands.as_slice()) {
                ("copy", [Operand::Constant(_, Number::Integer(count))]) => Some(*count),
                _ => None,
            })
            .collect()
    }

    /// The variable each descriptor count scales, traced back through the
    /// instructions that carry it.
    fn scaled(function: &Function) -> Vec<String> {
        let instructions: Vec<&Instruction> = function.blocks.iter().flat_map(|block| &block.instructions).collect();
        let defining = |value: u32| instructions.iter().find(|one| one.results.contains(&value)).copied();
        let is_count = |operand: &Operand| {
            matches!(operand, Operand::Value(value) if defining(*value)
                .is_some_and(|one| matches!(one.tag, Some(Tag::DescriptorField { field: Slot::Count(_), .. }))))
        };
        instructions
            .iter()
            .filter(|one| one.op == "mul" && is_count(&one.operands[1]))
            .map(|one| {
                let mut operand = one.operands[0].clone();
                loop {
                    match operand {
                        Operand::Value(value) => operand = defining(value).expect("defined").operands[0].clone(),
                        Operand::Place(place) => {
                            break function.places.iter().find(|one| one.id == place).expect("place").name.clone();
                        }
                        _ => panic!("a count scales something other than a variable"),
                    }
                }
            })
            .collect()
    }

    /// The element accesses that name an origin.
    fn originated(function: &Function) -> usize {
        function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .flat_map(|one| &one.operands)
            .filter(|operand| {
                matches!(operand, Operand::Indirect { base, inbounds: true, .. } if function.origins.contains_key(base))
            })
            .count()
    }

    const TWO_BY_FIVE: &str = "DEFINT A-Z\nSUB t\nDIM a(1, 4)\nx = a(1, 2)\nEND SUB\n";

    const SUBSCRIPTED: &str = "DEFINT A-Z\nSUB t\nDIM a(1, 4)\nx = a(i, j)\nEND SUB\n";

    /// The element formula read record p for the p-th source subscript, so
    /// a(i, j) was laid out row-major as i * 5 + j while B$DDIM, LBOUND and
    /// BC laid it out column-major, as BC's j * [a+12h] + i.
    #[test]
    fn test_a_column_major_element_scales_the_last_subscript_by_the_first_count() {
        let compiler = applied_to(SUBSCRIPTED);
        let t = function(&compiler, "t");
        assert_eq!((scaled(t), counts(t)), (vec!["J%".to_string()], vec![2]));
    }

    /// /R numbered a(i, j) as j * 5 + i. BC /R reverses the dimensions,
    /// i * [a+12h] + j, and B$DDIM's records with them.
    #[test]
    fn test_a_row_major_element_scales_the_first_subscript_by_the_last_count() {
        let compiler = applied_in(SUBSCRIPTED, true);
        let t = function(&compiler, "t");
        assert_eq!((scaled(t), counts(t)), (vec!["I%".to_string()], vec![5]));
    }

    /// DIM pushed its bounds last dimension first and REDIM in source order,
    /// so the same bounds filled the descriptor's records in opposite orders
    /// and an element after a REDIM read the other dimension's count.
    #[test]
    fn test_a_dim_and_a_redim_fill_the_same_records() {
        let compiler = applied_to("DEFINT A-Z\nSUB t\nDIM a(1, 4)\nREDIM a(1, 4)\nx = a(1, 2)\nEND SUB\n");
        assert_eq!(counts(function(&compiler, "t")), [2]);
    }

    /// The constants each +0Ah read became.
    fn origins(function: &Function) -> Vec<i64> {
        function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter(|one| matches!(one.tag, Some(Tag::DescriptorField { field: Slot::Origin, .. })))
            .filter_map(|one| match (one.op, one.operands.as_slice()) {
                ("copy", [Operand::Constant(_, Number::Integer(origin))]) => Some(*origin),
                _ => None,
            })
            .collect()
    }

    /// Worked by hand: a(i, j) is at ((j - 2) * 4 + i - 1) * 2, so +0Ah holds -(2 * 4 + 1) * 2.
    #[test]
    fn test_a_far_arrays_adjusted_offset_is_its_bounds_alone() {
        let one = applied_to("DEFINT A-Z\nSUB t\nREDIM a(-50 TO 50)\nx = a(3)\nEND SUB\n");
        assert_eq!(origins(function(&one, "t")), [100]);
        let two = applied_to("DEFINT A-Z\nSUB t\nREDIM a(1 TO 4, 2 TO 5)\nx = a(3, 4)\nEND SUB\n");
        assert_eq!(origins(function(&two, "t")), [-18]);
    }

    /// A near string array's data moves within DGROUP, and allocations that
    /// disagree share no offset.
    #[test]
    fn test_a_near_or_disagreeing_arrays_adjusted_offset_is_read() {
        let near = applied_to("DEFINT A-Z\nSUB t\nREDIM a$(-1 TO 3)\nx$ = a$(2)\nEND SUB\n");
        assert!(origins(function(&near, "t")).is_empty());
        let disagreeing = applied_to(
            "DEFINT A-Z\nREDIM SHARED a(9)\nSUB s\nREDIM a(5 TO 10)\nEND SUB\nSUB t\nx = a(6)\nEND SUB\n",
        );
        assert!(origins(function(&disagreeing, "t")).is_empty());
    }

    #[test]
    fn test_a_zero_based_element_names_its_origin() {
        let compiler = applied_to(TWO_BY_FIVE);
        assert_eq!(originated(function(&compiler, "t")), 1);
    }

    #[test]
    fn test_a_nonzero_lower_bound_anywhere_leaves_no_origin() {
        let compiler = applied_to(
            "DEFINT A-Z\nREDIM SHARED a(9)\nSUB s\nREDIM a(5 TO 10)\nEND SUB\nSUB t\nx = a(6)\nEND SUB\n",
        );
        assert_eq!(originated(function(&compiler, "t")), 0);
    }

    #[test]
    fn test_counts_that_differ_between_dims_are_not_folded() {
        let compiler = applied_to(
            "DEFINT A-Z\nREDIM SHARED a(1, 4)\nSUB s\nREDIM a(1, 7)\nEND SUB\nSUB t\nx = a(1, 2)\nEND SUB\n",
        );
        assert!(counts(function(&compiler, "t")).is_empty());
        assert_eq!(originated(function(&compiler, "t")), 1);
    }

    const PASSED: &str =
        "DEFINT A-Z\nDECLARE SUB t (q())\nREDIM a(1, 4)\nCALL t(a())\nx = a(1, 2)\nSUB t (q())\nx = q(1, 2)\nEND SUB\n";

    /// A SUB is public: another module may hand its parameter any array, and
    /// its REDIM reaches this module's array through the argument.
    #[test]
    fn test_a_public_procedures_array_parameter_has_no_shape() {
        let compiler = applied_to(PASSED);
        assert!(counts(function(&compiler, "t")).is_empty());
        assert!(counts(function(&compiler, "__main")).is_empty());
    }

    const HANDED_STATIC: &str =
        "DEFINT A-Z\nDECLARE SUB t (q())\nDIM a(1, 4)\nCALL t(a())\nSUB t (q())\nx = q(1, 2)\nEND SUB\n";

    /// A static array handed to a procedure left its parameter with no counts
    /// or origin.
    #[test]
    fn test_a_static_arrays_shape_reaches_its_parameter() {
        let compiler = applied_with(HANDED_STATIC, &Options { whole_program: true, ..Options::default() });
        let t = function(&compiler, "t");
        assert_eq!((counts(t), originated(t)), (vec![2], 1));
    }

    #[test]
    fn test_a_static_and_a_dynamic_array_of_other_counts_fold_none() {
        let source = HANDED_STATIC.replace("CALL t(a())\n", "CALL t(a())\nREDIM b(2, 4)\nCALL t(b())\n");
        let compiler = applied_with(&source, &Options { whole_program: true, ..Options::default() });
        let t = function(&compiler, "t");
        assert_eq!((counts(t), originated(t)), (vec![], 1));
    }

    /// Every SUB was public, so no array parameter had a shape even when the
    /// module was the whole program.
    #[test]
    fn test_a_whole_programs_array_parameter_has_its_arguments_shape() {
        let compiler = applied_with(PASSED, &Options { whole_program: true, ..Options::default() });
        let t = function(&compiler, "t");
        assert_eq!((counts(t), originated(t)), (vec![2], 1));
    }

    /// The constants each descriptor read of `field`'s kind became.
    fn folded(function: &Function, kind: fn(&Slot) -> bool) -> Vec<i64> {
        function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter(|one| matches!(&one.tag, Some(Tag::DescriptorField { field, .. }) if kind(field)))
            .filter_map(|one| match (one.op, one.operands.as_slice()) {
                ("copy", [Operand::Constant(_, Number::Integer(value))]) => Some(*value),
                _ => None,
            })
            .collect()
    }

    /// An array parameter's LBOUND read its rank from the descriptor, and its
    /// bound at an offset from it, though every argument has one rank and
    /// lower bound.
    #[test]
    fn test_a_parameters_rank_and_bound_are_its_arguments() {
        let source = "DEFINT A-Z\nDECLARE SUB t (q())\nREDIM a(3 TO n)\nCALL t(a())\nSUB t (q())\nx = LBOUND(q)\nEND SUB\n";
        let compiler = applied_with(source, &Options { whole_program: true, ..Options::default() });
        let t = function(&compiler, "t");
        assert_eq!(folded(t, |field| matches!(field, Slot::Rank)), [1]);
        assert_eq!(folded(t, |field| matches!(field, Slot::LowerOf(1))), [3]);
    }

    /// How many allocated tests of `function` became true.
    fn proven_allocated(function: &Function) -> usize {
        function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter(|one| matches!(one.tag, Some(Tag::Allocated { .. })) && one.op == "copy")
            .count()
    }

    const CHECKED_BOUND: &str = "DEFINT A-Z\nDECLARE FUNCTION u (q())\nREDIM a(5)\nREDIM b(7)\nPRINT u(a()); u(b())\n\
         FUNCTION u (q())\nu = UBOUND(q)\nEND FUNCTION\n";

    /// UBOUND of a parameter called B$UBND on an unallocated test although
    /// every caller DIMs the array it passes.
    #[test]
    fn test_a_parameter_every_caller_allocates_is_allocated() {
        let compiler = applied_with(CHECKED_BOUND, &Options { whole_program: true, checked_arrays: true, ..Options::default() });
        assert_eq!(proven_allocated(function(&compiler, "u")), 1);
    }

    /// One caller passing an ERASEd array, or a public procedure any other
    /// module may call, leaves the test.
    #[test]
    fn test_a_parameter_a_caller_erased_or_a_public_one_is_not() {
        let erased = CHECKED_BOUND.replace("PRINT u(a())", "ERASE a\nPRINT u(a())");
        let compiler = applied_with(&erased, &Options { whole_program: true, checked_arrays: true, ..Options::default() });
        assert_eq!(proven_allocated(function(&compiler, "u")), 0);
        let compiler = applied_with(CHECKED_BOUND, &Options { checked_arrays: true, ..Options::default() });
        assert_eq!(proven_allocated(function(&compiler, "u")), 0);
    }

    #[test]
    fn test_an_array_handed_to_an_undefined_procedure_has_no_shape() {
        let compiler = applied_to(
            "DEFINT A-Z\nDECLARE SUB u (q())\nSUB t\nDIM a(1, 4)\nCALL u(a())\nx = a(1, 2)\nEND SUB\n",
        );
        assert!(counts(function(&compiler, "t")).is_empty());
        assert_eq!(originated(function(&compiler, "t")), 0);
    }
}
