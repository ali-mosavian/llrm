//! What the QB grammar leaves for later passes.

use lalrspike::parse_qb;
use lalrspike::qb_tree::Tree;
use qbfront::Dialect;
use qbfront::front::scan::scan as lex;

fn parse(source: &str) -> Result<Tree, String> {
    let tokens = lex(&format!("{source}\n"), Dialect::VbDos).map_err(|error| error.message)?;
    parse_qb(&tokens)
}

fn rules(source: &str) -> Vec<&'static str> {
    let mut out = Vec::new();
    parse(source).unwrap_or_else(|error| panic!("{source}: {error}")).rules(&mut out);
    out
}

#[test]
fn a_name_with_parentheses_is_one_node_whatever_it_names() {
    assert!(rules("x = a(1) + f(2)").iter().filter(|&&rule| rule == "Apply").count() == 2);
    assert!(rules("a(i, j) = 0").contains(&"Apply"));
    assert!(rules("r.f(1).g = 0").contains(&"FieldApply"));
}

#[test]
fn a_call_without_call_is_told_by_its_arguments() {
    assert!(rules("foo a, b").contains(&"CallArgs"));
    assert!(rules("foo").contains(&"Call"));
    assert!(rules("foo (a), b").contains(&"CallAppliedList"));
    assert!(rules("foo(a)").contains(&"CallApplied"));
}

#[test]
fn a_name_and_a_colon_at_the_line_start_is_a_label() {
    assert!(rules("start: x = 1").contains(&"Label"));
    assert!(rules("10 x = 1").contains(&"Label"));
    // after the first statement `name:` is a call
    assert!(!rules("x = 1: foo: y = 2").contains(&"Label"));
}

#[test]
fn if_then_else_is_a_flat_list_for_a_later_pass_to_nest() {
    let flat = rules("IF a THEN IF b THEN x = 1 ELSE y = 2 ELSE z = 3");
    assert_eq!(flat.iter().filter(|&&rule| rule == "IF").count(), 2);
    assert_eq!(flat.iter().filter(|&&rule| rule == "ELSE").count(), 2);
    assert!(rules("IF a THEN 100 ELSE 200").contains(&"ThenLine"));
    assert!(rules("IF a GOTO 100").contains(&"IFGOTO"));
    assert!(rules("IF a THEN").contains(&"IF"));
}

#[test]
fn print_items_need_no_separator_between_them() {
    assert!(parse("PRINT a; b, TAB(3) c USING \"##\"; d").is_ok());
    // QB reads `PRINT a "x" b` as `a; "x"; b`; a sign or parenthesis continues the item before
    assert!(parse("PRINT a \"x\" b").is_ok());
    assert!(parse("PRINT (").is_err());
}

#[test]
fn a_line_number_and_a_label_are_the_expressions_they_look_like() {
    assert!(rules("RUN 100").contains(&"RUN"));
    assert!(rules("RESUME 0").contains(&"RESUME"));
    assert!(rules("OPTION BASE 1").contains(&"OPTIONBASE"));
}

#[test]
fn on_takes_an_event_a_computed_jump_or_a_trap_the_expression_spells() {
    assert!(rules("ON ERROR GOTO handler").contains(&"ONERROR"));
    assert!(rules("ON TIMER(5) GOSUB tick").contains(&"ONEVENT"));
    assert!(rules("ON PEN GOSUB tick").contains(&"ONEVENT"));
    assert!(rules("ON n GOTO 1, 2, 3").contains(&"ON"));
    // PLAY(n) is a function: the pass that knows the trap spelling reads it
    assert!(rules("ON PLAY(3) GOSUB tick").contains(&"ON"));
}

#[test]
fn a_member_may_be_named_as_a_reserved_word() {
    assert!(rules("name AS STRING * 16").contains(&"Member"));
    assert!(rules("pos AS Vec2i").contains(&"Member"));
    assert!(rules("padding(29) AS INTEGER").contains(&"Member"));
    assert!(parse("v.pos.x = v.pos.y").is_ok());
}

#[test]
fn greedy_optional_arguments_are_one_nesting() {
    assert!(parse("PAINT (1, 1), 2, 3, 4").is_ok());
    assert!(parse("VIEW (1, 1)-(2, 2), 1, 2").is_ok());
    assert!(parse("PUT (1, 1), sprite%, PSET").is_ok());
    assert!(parse("LINE (1, 1)-(2, 2), 4, BF").is_ok());
}

#[test]
fn syntax_the_grammar_does_not_have_is_an_error() {
    assert!(parse("FOR = 1").is_err());
    assert!(parse("x = ").is_err());
    assert!(parse("PRINT (").is_err());
}

#[test]
fn dialect_additions_are_productions() {
    assert!(rules("OPTION EXPLICIT").contains(&"OPTIONEXT"));
    assert!(rules("ON LOCAL ERROR GOTO handler").contains(&"ONLOCALERROR"));
    assert!(rules("DECLARE FUNCTION f CDECL ALIAS \"g\" (a AS INTEGER) AS INTEGER").contains(&"DECLARE"));
    assert!(rules("PRIVATE FUNCTION f (a AS INTEGER) AS INTEGER").contains(&"FUNCTION"));
}
