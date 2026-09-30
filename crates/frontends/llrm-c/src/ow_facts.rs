//! What the language states, as Open Watcom's front end and our patch to it
//! tell it: the one table from their words to facts (`llrm_mir::facts`).
//!
//! The patched front end speaks in terms of the language (`restrict`), never
//! in llrm's; `translate` states whatever this table says and holds no code
//! of its own for any fact. A stream from a build of another version is
//! refused, not misread.

use llrm_mir::facts::{Effect, Fact};

use crate::hir::{Node, Unit, Unsupported, handle};

/// The version of the `CGFact` record the shim writes.
pub const VERSION: &str = "v1";

/// The fact a term of the language states of a parameter.
pub fn param_fact(term: &str) -> Option<Fact> {
    match term {
        "restrict" => Some(Fact::NoAlias),
        _ => None,
    }
}

/// Open Watcom's call class (`cg/h/cgauxcc.h`) bits that state a fact about
/// the routine called.
const ABORTS: i64 = 0x2;
const NORETURN: i64 = 0x4;
const NO_MEMORY_READ: i64 = 0x100;
const NO_MEMORY_CHANGED: i64 = 0x200;

/// The facts a routine's call class states: it does not return, and what it
/// does not do to memory (`#pragma aux ... nomemory`).
pub fn of_call_class(class: i64) -> Vec<Fact> {
    let mut facts = Vec::new();
    if class & (ABORTS | NORETURN) != 0 {
        facts.push(Fact::NoReturn);
    }
    match (class & NO_MEMORY_READ != 0, class & NO_MEMORY_CHANGED != 0) {
        (true, true) => facts.push(Fact::Memory(Effect::None)),
        (false, true) => facts.push(Fact::Memory(Effect::Read)),
        (true, false) => facts.push(Fact::Memory(Effect::Write)),
        (false, false) => {}
    }
    facts
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

    /// Restrict states `NoAlias` of the parameter it names, and of no other.
    #[test]
    fn a_call_class_states_noreturn_and_what_memory_a_routine_leaves_alone() {
        assert_eq!(of_call_class(0x80), Vec::<Fact>::new());
        assert_eq!(of_call_class(0x84), vec![Fact::NoReturn]);
        assert_eq!(of_call_class(0x82), vec![Fact::NoReturn]);
        assert_eq!(of_call_class(0x380), vec![Fact::Memory(Effect::None)]);
        assert_eq!(of_call_class(0x280), vec![Fact::Memory(Effect::Read)]);
        assert_eq!(of_call_class(0x180), vec![Fact::Memory(Effect::Write)]);
    }

    #[test]
    fn restrict_states_noalias_of_the_parameter_it_names() {
        let mut unit = Unit::default();
        unit.nodes.insert(1, Node { call: "CGFEName".to_owned(), args: vec!["y5".to_owned(), "TY_POINTER".to_owned()] });
        unit.nodes.insert(2, node(&["v1", "n1", "restrict"]));
        assert_eq!(of_param(&unit, 5), vec![Fact::NoAlias]);
        assert_eq!(of_param(&unit, 6), Vec::<Fact>::new());
    }
}
