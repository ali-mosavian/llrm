//! Array shapes over the whole module.
//!
//! Every DIM, REDIM and static array's fixed bounds that can reach a
//! descriptor is known here: its own procedure's, and through whole-array
//! arguments, its callers' and callees'.
//! Where all of them agree, the shape is a fact: constant dimension counts
//! replace their descriptor reads, and a zero-based array's descriptor offset
//! is its first byte, which each element access records as its origin.
//!
//! A descriptor another module can reach -- an external place, or an array
//! parameter of an externally callable procedure -- has no fact, nor has one
//! handed to a callee this module does not define.

use std::collections::BTreeMap;

use super::tags::{Passing, Slot, Tag};
use super::{Compiler, Function, Number, Operand, Place};

/// A descriptor, independently of the value that points to it.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Identity {
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
    zero_based: bool,
    counts: Option<Vec<i64>>,
    allocations: usize,
}

impl Known {
    /// Fold in one allocation's per-record bounds.
    fn merged(&mut self, records: &[(Operand, Operand)]) {
        let constant = |operand: &Operand| match operand {
            Operand::Constant(_, Number::Integer(value)) => Some(*value),
            _ => None,
        };
        let zero_based = records.iter().all(|(lower, _)| constant(lower) == Some(0));
        let counts: Option<Vec<i64>> = records
            .iter()
            .map(|(lower, upper)| Some(constant(upper)? - constant(lower)? + 1))
            .collect();
        if self.allocations == 0 {
            (self.zero_based, self.counts) = (zero_based, counts);
        } else {
            self.zero_based &= zero_based;
            if self.counts != counts {
                self.counts = None;
            }
        }
        self.allocations += 1;
    }

    /// Whether anything here is a fact.
    fn proven(&self) -> bool {
        !self.unknown && self.allocations != 0
    }
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
fn identity(function: &Function, place: &Place) -> Identity {
    match place.storage {
        "local" => Identity::Local(function.id, place.id),
        _ => Identity::Global(place.symbol, place.offset),
    }
}

/// Each descriptor pointer value of `function` and the descriptor it names.
fn pointers(function: &Function) -> BTreeMap<u32, Identity> {
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
    let mut shapes: Vec<(Identity, &[(Operand, Operand)])> = statics.into_iter().collect();
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
                    Some(identity) => shapes.push((*identity, &shape.records)),
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
    let allocated: Vec<Identity> = shapes.iter().map(|(identity, _)| *identity).collect();
    unknown.extend(handed.into_iter().filter(|identity| {
        !matches!(identity, Identity::Parameter(..)) && !allocated.contains(identity)
    }));
    let mut out: BTreeMap<Identity, Known> = BTreeMap::new();
    for (identity, records) in shapes {
        let root = classes.root(identity);
        out.entry(root).or_default().merged(records);
    }
    for identity in unknown {
        let root = classes.root(identity);
        out.entry(root).or_default().unknown = true;
    }
    (classes, out)
}

/// Fold each proven fact into the HIR.
pub(super) fn applied(compiler: &mut Compiler) {
    let (mut classes, known) = known(compiler);
    for function in &mut compiler.functions {
        let pointers = pointers(function);
        let mut fact = |descriptor: u32| {
            let identity = pointers.get(&descriptor)?;
            known.get(&classes.root(*identity)).filter(|one| one.proven()).cloned()
        };
        let types: BTreeMap<u32, u32> = function.values.iter().copied().collect();

        for block in &mut function.blocks {
            for one in &mut block.instructions {
                match one.tag {
                    Some(Tag::DescriptorField { descriptor, field: Slot::Count(record) }) => {
                        let Some(counts) = fact(descriptor).and_then(|fact| fact.counts) else {
                            continue;
                        };
                        let Some(&count) = counts.get(record) else {
                            continue;
                        };
                        one.op = "copy";
                        one.operands = vec![Operand::Constant(types[&one.results[0]], Number::Integer(count))];
                    }
                    Some(Tag::ElementOffset { descriptor, origin: Some(origin) }) => {
                        if fact(descriptor).is_some_and(|fact| fact.zero_based) {
                            function.origins.extend(one.results.iter().map(|result| (*result, origin)));
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
                if let Some(origin) = function.origins.get(offset) {
                    concatenated.push((*result, *origin));
                }
            }
        }
        function.origins.extend(concatenated);
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

    #[test]
    fn test_an_array_handed_to_an_undefined_procedure_has_no_shape() {
        let compiler = applied_to(
            "DEFINT A-Z\nDECLARE SUB u (q())\nSUB t\nDIM a(1, 4)\nCALL u(a())\nx = a(1, 2)\nEND SUB\n",
        );
        assert!(counts(function(&compiler, "t")).is_empty());
        assert_eq!(originated(function(&compiler, "t")), 0);
    }
}
