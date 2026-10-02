//! What the language states, as Open Watcom's front end and our patch to it
//! tell it: the one table from their words to facts (`llrm_mir::facts`).
//!
//! The patched front end speaks in terms of the language (`restrict`), never
//! in llrm's; `translate` states whatever this table says and holds no code
//! of its own for any fact. A stream from a build of another version is
//! refused, not misread.

use llrm_mir::facts::{Effect, Fact, Inlining};

use crate::hir::{Node, Unit, Unsupported, handle};

/// The version of the `CGFact` record the shim writes.
pub const VERSION: &str = "v1";

/// The terms the patched front end and the shim can state: the one list
/// they read, `toolchain/owshim/qbfacts.def`.
const TERMS: &str = include_str!("../../../../toolchain/owshim/qbfacts.def");

pub fn terms() -> Vec<&'static str> {
    TERMS.lines().filter_map(|line| line.strip_prefix("QBFACT(")).filter_map(|rest| rest.split(',').next()).map(str::trim).collect()
}

/// The fact a term of the language states of a parameter.
pub fn param_fact(term: &str) -> Option<Fact> {
    match term {
        "restrict" => Some(Fact::NoAlias),
        _ => None,
    }
}

/// What becomes of one bit of Open Watcom's call class (`cg/h/cgauxcc.h`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Bit {
    /// States a fact about the routine (`NoReturn`, `Memory`); see `of_call_class`.
    Fact,
    /// Part of how the routine is called; read where the call is made.
    Abi(&'static str),
    /// Changes what the code after a call means, so no fact: a field of the routine.
    Meaning(&'static str),
    /// Changes what the program means and has no carrier yet: the compile stops.
    Refused(&'static str),
    /// Has no meaning for this target.
    Ignored(&'static str),
}

const ABORTS: i64 = 0x2;
const NORETURN: i64 = 0x4;
const MAKE_CALL_INLINE: i64 = 0x10;
const NO_MEMORY_READ: i64 = 0x100;
const NO_MEMORY_CHANGED: i64 = 0x200;

/// Every bit the call class has, and what is done with it: a bit not in this
/// table is refused, not dropped.
const CLASS: [(i64, &str, Bit); 11] = [
    (0x1, "REVERSE_PARMS", Bit::Abi("Symbol::in_order")),
    (ABORTS, "ABORTS", Bit::Fact),
    (NORETURN, "NORETURN", Bit::Fact),
    (0x8, "PARMS_BY_ADDRESS", Bit::Refused("arguments passed by address have no carrier")),
    (MAKE_CALL_INLINE, "MAKE_CALL_INLINE", Bit::Fact),
    (0x20, "HAS_VARARGS", Bit::Abi("Symbol::variadic")),
    (0x40, "SETJMP_KLUGE", Bit::Meaning("Callable::returns_twice")),
    (0x80, "CALLER_POPS", Bit::Abi("the call's stack cleanup")),
    (NO_MEMORY_READ, "NO_MEMORY_READ", Bit::Fact),
    (NO_MEMORY_CHANGED, "NO_MEMORY_CHANGED", Bit::Fact),
    (0x400, "DLL_EXPORT", Bit::Ignored("a Windows export")),
];

/// The facts a routine's call class states: it does not return, what it
/// does not do to memory (`#pragma aux ... nomemory`), and that the language
/// marked it inline (our patched front end sets `MAKE_CALL_INLINE` of what is
/// declared `inline`; it is a hint, nothing asks the code generator for a body). A bit that changes
/// meaning and has no carrier, or that this table does not know, refuses.
pub fn of_call_class(class: i64) -> Result<Vec<Fact>, Unsupported> {
    let known = CLASS.iter().fold(0, |all, (bit, ..)| all | bit);
    if class & !known != 0 {
        return Err(Unsupported(format!("call class bits {:#x} are in no table here", class & !known)));
    }
    if let Some((_, name, Bit::Refused(why))) = CLASS.iter().find(|(bit, _, how)| class & bit != 0 && matches!(how, Bit::Refused(_))) {
        return Err(Unsupported(format!("call class {name}: {why}")));
    }
    let mut facts = Vec::new();
    if class & (ABORTS | NORETURN) != 0 {
        facts.push(Fact::NoReturn);
    }
    if class & MAKE_CALL_INLINE != 0 {
        facts.push(Fact::Inline(Inlining::Hint));
    }
    match (class & NO_MEMORY_READ != 0, class & NO_MEMORY_CHANGED != 0) {
        (true, true) => facts.push(Fact::Memory(Effect::None)),
        (false, true) => facts.push(Fact::Memory(Effect::Read)),
        (true, false) => facts.push(Fact::Memory(Effect::Write)),
        (false, false) => {}
    }
    Ok(facts)
}

/// Whether the routine may return a second time (`setjmp`'s class bit).
pub fn returns_twice(class: i64) -> bool {
    class & 0x40 != 0
}

/// A `CGFact` node's version and term, checked: `CGFact v1 n7 restrict`.
pub fn check(node: &Node) -> Result<(), Unsupported> {
    match &node.args[..] {
        [version, _, term] if version == VERSION && param_fact(term).is_some() => Ok(()),
        [version, ..] if version != VERSION => Err(Unsupported(format!("the stream's facts are {version}, this reads {VERSION}: rebuild wccq"))),
        _ => Err(Unsupported(format!("a fact the stream states that no table here knows: {}", node.args.join(" ")))),
    }
}

/// The facts the language states of parameter `symbol`: each `CGFact` of
/// its name.
pub fn of_param(unit: &Unit, symbol: i64) -> Vec<Fact> {
    let mut facts = Vec::new();
    for node in unit.nodes.values().filter(|node| node.call == "CGFact") {
        let [_, inner, term] = &node.args[..] else { continue };
        let named = unit.nodes.get(&handle(inner)).is_some_and(|one| one.call == "CGFEName" && handle(&one.args[0]) == symbol);
        if let Some(fact) = param_fact(term).filter(|_| named) {
            if !facts.contains(&fact) {
                facts.push(fact);
            }
        }
    }
    facts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(args: &[&str]) -> Node {
        Node { call: "CGFact".to_owned(), args: args.iter().map(|one| (*one).to_owned()).collect() }
    }

    /// A stream from a stale `wccq` spoke `CGAttr n 3`, which was read as
    /// restrict or not at all; a fact of another version or term is refused
    /// by name, so a stale build fails loudly.
    #[test]
    fn a_fact_of_another_version_or_term_is_refused() {
        assert!(check(&node(&["v1", "n7", "restrict"])).is_ok());
        assert!(check(&node(&["v2", "n7", "restrict"])).unwrap_err().0.contains("rebuild wccq"));
        assert!(check(&node(&["v1", "n7", "noreturn"])).unwrap_err().0.contains("no table here knows"));
    }

    /// A term added to the list the front end and shim read, and not here,
    /// would be refused as unknown only when a program used it.
    #[test]
    fn every_term_the_front_end_can_state_has_a_fact() {
        assert_eq!(terms(), vec!["restrict"]);
        for term in terms() {
            assert!(param_fact(term).is_some(), "{term}");
        }
    }

    /// Restrict states `NoAlias` of the parameter it names, and of no other.
    #[test]
    fn a_call_class_states_noreturn_and_what_memory_a_routine_leaves_alone() {
        assert_eq!(of_call_class(0x80).unwrap(), Vec::<Fact>::new());
        assert_eq!(of_call_class(0x84).unwrap(), vec![Fact::NoReturn]);
        assert_eq!(of_call_class(0x82).unwrap(), vec![Fact::NoReturn]);
        assert_eq!(of_call_class(0x380).unwrap(), vec![Fact::Memory(Effect::None)]);
        assert_eq!(of_call_class(0x280).unwrap(), vec![Fact::Memory(Effect::Read)]);
        assert_eq!(of_call_class(0x180).unwrap(), vec![Fact::Memory(Effect::Write)]);
    }

    #[test]
    fn restrict_states_noalias_of_the_parameter_it_names() {
        let mut unit = Unit::default();
        unit.nodes.insert(1, Node { call: "CGFEName".to_owned(), args: vec!["y5".to_owned(), "TY_POINTER".to_owned()] });
        unit.nodes.insert(2, node(&["v1", "n1", "restrict"]));
        assert_eq!(of_param(&unit, 5), vec![Fact::NoAlias]);
        assert_eq!(of_param(&unit, 6), Vec::<Fact>::new());
    }

    /// A call class bit that changes meaning and has no carrier, or that no
    /// table knows, was dropped without a word: a `setjmp` that returns twice
    /// compiled as though it returned once.
    #[test]
    fn a_call_class_bit_with_no_carrier_stops_the_compile() {
        // A routine that returns twice is a field of the callable, not a refusal.
        assert_eq!(of_call_class(0x40).unwrap(), Vec::<Fact>::new());
        assert!(returns_twice(0x40) && !returns_twice(0x80));
        assert!(of_call_class(0x88).unwrap_err().0.contains("PARMS_BY_ADDRESS"));
        assert!(of_call_class(0x800).unwrap_err().0.contains("no table"));
        // The ABI bits and the ignored one pass; the inline mark is a hint.
        assert_eq!(of_call_class(0x1 | 0x20 | 0x80 | 0x400).unwrap(), Vec::<Fact>::new());
        assert_eq!(of_call_class(0x10).unwrap(), vec![Fact::Inline(Inlining::Hint)]);
        // Each bit is in the table once.
        let mut bits: Vec<i64> = CLASS.iter().map(|one| one.0).collect();
        bits.sort_unstable();
        bits.dedup();
        assert_eq!(bits.len(), CLASS.len());
    }
}
