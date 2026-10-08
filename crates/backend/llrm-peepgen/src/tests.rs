use super::*;
use automaton::{Key, Node};

/// m16's forms: the family's, then its own.
fn forms() -> String {
    let read = |name: &str| std::fs::read_to_string(format!("{}/../../target/{name}/src/instructions/x86.instr", env!("CARGO_MANIFEST_DIR"))).unwrap();
    format!("{}\n{}", read("llrm-x86"), read("llrm-x86-m16"))
}

fn table() -> table::Table {
    table::load(&forms(), "x86.instr").unwrap()
}

fn compiled(rules: &str) -> Result<Vec<automaton::Automaton>, String> {
    compile(&table(), rules, "test.peep").map(|(_, automata)| automata)
}

#[test]
fn the_rules_llrm_builds_with_compile() {
    let rules = format!("{}\n{}", include_str!("../../../target/llrm-x86/src/isel/peephole.peep"), include_str!("../../../target/llrm-x86-m16/src/isel/peephole.peep"));
    let made = generate(&forms(), "x86.instr", &rules, "peephole.peep");
    assert!(made.is_ok(), "{}", made.err().unwrap_or_default());
}

/// A rule reading an operand it never bound would have been emitted as
/// Rust that fails to compile far from the rule; it is refused at its line.
#[test]
fn a_malformed_rule_is_refused_with_its_line() {
    let refused = |rules: &str| compiled(rules).err().expect("refused");
    let group = "group g\n    walk each\n";
    assert_eq!(
        refused(&format!("{group}rule r\n    match add d:reg, d, #1\n    rewrite @0: inc e, e\n")),
        "test.peep:3: rule r: e is not bound"
    );
    assert_eq!(
        refused(&format!("{group}rule r\n    match add d, d, #1\n    rewrite @0: inc d, d\n")),
        "test.peep:3: rule r: d is not bound; give its kind, as d:reg"
    );
    assert_eq!(
        refused(&format!("{group}rule r\n    match addd d:reg, d, #1\n    rewrite @0: inc d, d\n")),
        "test.peep:3: rule r: unknown mnemonic or set 'addd'"
    );
    assert_eq!(
        refused("group g\n    walk gap\nrule r\n    match mov t:reg, s:reg ; mov s, t ; ... ok(*) ; mov s, t\n    rewrite @0 ; @1 ; @2\n"),
        "test.peep:3: rule r: a gap '...' comes only after the first instruction"
    );
}

/// `add r,1` and `add r,2` differ only in the immediate: one state tests
/// the operation, the operand counts and kinds for both, and the automaton
/// branches only where they differ.
#[test]
fn rules_sharing_a_prefix_share_its_states() {
    let rules = "group g\n    walk each\n\
                 rule one\n    match add d:reg, d, #1\n    rewrite @0: inc d, d\n\
                 rule two\n    match add d:reg, d, #2\n    rewrite @0: dec d, d\n";
    let automata = compiled(rules).unwrap();
    let automaton = &automata[0];
    let testing = |key: Key| automaton.nodes.iter().filter(|node| matches!(node, Node::Switch { key: one, .. } if *one == key)).count();
    assert_eq!(testing(Key::Op(0)), 1);
    assert_eq!(testing(Key::Dests(0)), 1);
    assert_eq!(testing(Key::Sources(0)), 1);
    let alone = |rule: &str| compiled(&format!("group g\n    walk each\n{rule}")).unwrap()[0].states();
    let one = alone("rule one\n    match add d:reg, d, #1\n    rewrite @0: inc d, d\n");
    // Two only adds its own mnemonic test and accepting state.
    assert_eq!(automaton.states(), one + 2);
}

/// A definition is found by the held value a window instruction binds; one
/// for a register would search a value no instruction defines.
#[test]
fn a_definition_of_no_held_value_is_refused() {
    let refused = compiled(
        "group g\n    walk window\nrule r\n    match push v:reg ; def v: mov v, c:mem\n    rewrite @0: =@0 c ; drop @1\n",
    )
    .err()
    .expect("refused");
    assert_eq!(refused, "test.peep:3: rule r: def v: v is not a held value the window binds");
}
