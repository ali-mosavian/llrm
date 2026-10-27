//! What the Nib grammar leaves for later passes, and the one lookahead it
//! cannot do itself.

use lalrspike::ast::*;
use lalrspike::parse_nib_source;

fn body(source: &str) -> Vec<Stmt> {
    let module = parse_nib_source(&format!("fn main() -> i16:\n{}\n", indent(source))).expect("parses");
    match module.items.into_iter().next().unwrap().decl {
        Decl::Function { function, .. } => function.body.unwrap().0,
        other => panic!("{other:?}"),
    }
}

fn indent(source: &str) -> String {
    source.lines().map(|line| format!("    {line}")).collect::<Vec<_>>().join("\n")
}

fn expr(source: &str) -> Expr {
    match body(source).remove(0) {
        Stmt::Expr(expr) => expr,
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_call_a_cast_and_a_construction_are_one_node() {
    for source in ["f(x)", "i16(x)", "Point(1, 2)", "geo.Point(1, 2)", "x.len()"] {
        assert!(matches!(expr(source), Expr::Apply(..)), "{source}");
    }
}

#[test]
fn an_index_a_generic_call_and_an_array_type_are_one_node() {
    assert!(matches!(expr("a[i]"), Expr::Subscript(..)));
    let Expr::Apply(callee, _) = expr("f[i16](x)") else { panic!() };
    assert!(matches!(*callee, Expr::Subscript(..)));
    assert!(matches!(expr("a[1:2]"), Expr::Subscript(_, ref index) if matches!(index[0], Index::Slice(..))));
}

#[test]
fn a_pointer_type_in_a_type_argument_is_a_type() {
    let Expr::Apply(callee, _) = expr("block.cast[*near mut u8]()") else { panic!() };
    let Expr::Subscript(_, index) = *callee else { panic!() };
    assert!(matches!(index[0], Index::At(Expr::TypeOnly(_))));
}

#[test]
fn an_assignment_target_is_any_expression_until_a_pass_checks_it() {
    for source in ["x = 1", "a[i] = 1", "p.x += 1", "*p = 1"] {
        assert!(matches!(body(source).remove(0), Stmt::Assign(..)), "{source}");
    }
    // `f(x) = 1` parses; refusing it is the semantic pass's.
    assert!(matches!(body("f(x) = 1").remove(0), Stmt::Assign(Expr::Apply(..), ..)));
}

#[test]
fn a_for_source_keeps_its_borrow_for_a_pass() {
    let Stmt::For { source, .. } = body("for x in &mut v:\n    pass()").remove(0) else { panic!() };
    assert!(matches!(source, Expr::Unary("&mut", _)));
}

#[test]
fn a_question_mark_before_a_colon_opens_a_conditional() {
    assert!(matches!(expr("a ? b : c"), Expr::Ternary(..)));
    assert!(matches!(expr("a ? -b : c"), Expr::Ternary(..)));
    assert!(matches!(expr("f()? - 1"), Expr::Binary("-", ..)));
    assert!(matches!(expr("f()?"), Expr::Try(_)));
}

#[test]
fn an_unexpected_token_is_an_error() {
    assert!(parse_nib_source("fn main() -> i16:\n    let = 1\n").is_err());
    assert!(parse_nib_source("fn main(:\n").is_err());
}
