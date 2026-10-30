//! The expression language of `tools/scev/oracle.cpp`, evaluated with these
//! folds, so that one input line gives LLVM's text and ours to compare. See
//! the grammar at the top of the oracle.

use llrm_mir::module::{Operand, ValueDef};

use super::ap::Ap;
use super::expr::{Scev, Ty, Use, flags};
use super::print::{Labels, type_text};
use super::{Cx, Flags, ScalarEvolution};

/// A parsed s-expression.
#[derive(Debug)]
pub enum Node {
    Atom(String),
    List(Vec<Node>),
}

/// Parses one s-expression.
pub fn parse(text: &str) -> Node {
    fn go(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Node {
        while chars.peek() == Some(&' ') {
            chars.next();
        }
        if chars.peek() == Some(&'(') {
            chars.next();
            let mut kids = Vec::new();
            loop {
                while chars.peek() == Some(&' ') {
                    chars.next();
                }
                if chars.peek() == Some(&')') {
                    chars.next();
                    break;
                }
                kids.push(go(chars));
            }
            return Node::List(kids);
        }
        let mut atom = String::new();
        while let Some(&c) = chars.peek() {
            if c == ' ' || c == ')' {
                break;
            }
            atom.push(c);
            chars.next();
        }
        Node::Atom(atom)
    }
    go(&mut text.chars().peekable())
}

fn parse_flags(text: &str) -> u8 {
    if text == "-" {
        return flags::NONE;
    }
    text.split(',').fold(flags::NONE, |f, one| {
        f | match one {
            "nw" => flags::NW,
            "nuw" => flags::NUW,
            "nsw" => flags::NSW,
            _ => flags::NONE,
        }
    })
}

fn parse_scev_flags(text: &str) -> Flags {
    let (expr, use_) = text.split_once(':').expect("EXPR:USE flags");
    Flags { expr: parse_flags(expr), use_: parse_flags(use_) }
}

impl ScalarEvolution {
    /// Evaluates `node`, which is a list.
    pub fn eval(
        &mut self,
        cx: &Cx,
        node: &Node,
    ) -> Use {
        let Node::List(kids) = node else { panic!("an expression is a list") };
        let Node::Atom(op) = &kids[0] else { panic!("an operator") };
        let atom = |at: usize| -> &str {
            match &kids[at] {
                Node::Atom(text) => text,
                Node::List(_) => panic!("an atom"),
            }
        };
        let width = |at: usize| Ty::Int(atom(at).parse().expect("a width"));
        let args =
            |se: &mut Self, from: usize| -> Vec<Use> { kids[from..].iter().map(|kid| se.eval(cx, kid)).collect() };
        match op.as_str() {
            "c" => {
                let w: u32 = atom(1).parse().expect("a width");
                let value: u128 = atom(2).parse().expect("a value");
                Use::of(self.get_constant(Ap::new(w, value)))
            }
            "u" => {
                let at: usize = atom(1).parse().expect("an argument");
                let id = cx.function.parameters()[at];
                Use::of(self.get_unknown(cx, Operand::Value(id)))
            }
            "use" => {
                let inner = self.eval(cx, &kids[2]);
                // Use flags need an expression that can carry them; others
                // pass through.
                if matches!(
                    self.node(inner.scev).kind,
                    super::expr::Kind::Add(_) | super::expr::Kind::Mul(_) | super::expr::Kind::AddRec(..)
                ) {
                    self.with_use_flags(inner.scev, parse_flags(atom(1)))
                } else {
                    inner
                }
            }
            "trunc" => {
                let operand = self.eval(cx, &kids[2]);
                Use::of(self.get_truncate_expr(cx, operand, width(1), 0))
            }
            "zext" => {
                let operand = self.eval(cx, &kids[2]);
                Use::of(self.get_zero_extend_expr(cx, operand, width(1), 0))
            }
            "sext" => {
                let operand = self.eval(cx, &kids[2]);
                Use::of(self.get_sign_extend_expr(cx, operand, width(1), 0))
            }
            "anyext" => {
                let operand = self.eval(cx, &kids[2]);
                Use::of(self.get_any_extend_expr(cx, operand, width(1)))
            }
            "add" => {
                let ops = args(self, 2);
                self.get_add_expr(cx, ops, parse_scev_flags(atom(1)), 0)
            }
            "mul" => {
                let ops = args(self, 2);
                self.get_mul_expr(cx, ops, parse_scev_flags(atom(1)), 0)
            }
            "udiv" => {
                let (l, r) = (self.eval(cx, &kids[1]), self.eval(cx, &kids[2]));
                Use::of(self.get_udiv_expr(cx, l, r))
            }
            "udivexact" => {
                let (l, r) = (self.eval(cx, &kids[1]), self.eval(cx, &kids[2]));
                Use::of(self.get_udiv_exact_expr(cx, l, r))
            }
            "urem" => {
                let (l, r) = (self.eval(cx, &kids[1]), self.eval(cx, &kids[2]));
                Use::of(self.get_urem_expr(cx, l, r))
            }
            "minus" => {
                let (l, r) = (self.eval(cx, &kids[1]), self.eval(cx, &kids[2]));
                Use::of(self.get_minus_scev(cx, l, r, flags::NONE, 0))
            }
            "neg" => {
                let operand = self.eval(cx, &kids[1]);
                Use::of(self.get_negative_scev(cx, operand.scev, flags::NONE))
            }
            "not" => {
                let operand = self.eval(cx, &kids[1]);
                Use::of(self.get_not_scev(cx, operand.scev))
            }
            "smax" => {
                let ops = args(self, 1);
                Use::of(self.get_smax_expr(cx, ops))
            }
            "umax" => {
                let ops = args(self, 1);
                Use::of(self.get_umax_expr(cx, ops))
            }
            "smin" => {
                let ops = args(self, 1);
                Use::of(self.get_smin_expr(cx, ops))
            }
            "umin" => {
                let ops = args(self, 1);
                Use::of(self.get_umin_expr(cx, ops, false))
            }
            "sequmin" => {
                let ops = args(self, 1);
                Use::of(self.get_umin_expr(cx, ops, true))
            }
            "addrec" => {
                let at: usize = atom(2).parse().expect("a loop");
                let header = cx.loops.get_loops_in_preorder()[at].get_header();
                let ops = args(self, 3);
                self.get_add_rec_expr(cx, ops, header, parse_scev_flags(atom(1)))
            }
            "tozext" => {
                let operand = self.eval(cx, &kids[2]);
                Use::of(self.get_truncate_or_zero_extend(cx, operand.scev, width(1), 0))
            }
            "tosext" => {
                let operand = self.eval(cx, &kids[2]);
                Use::of(self.get_truncate_or_sign_extend(cx, operand.scev, width(1), 0))
            }
            "totrunc" => {
                let operand = self.eval(cx, &kids[2]);
                Use::of(self.get_truncate_or_noop(cx, operand.scev, width(1)))
            }
            "noopzext" => {
                let operand = self.eval(cx, &kids[2]);
                Use::of(self.get_noop_or_zero_extend(cx, operand.scev, width(1)))
            }
            "noopsext" => {
                let operand = self.eval(cx, &kids[2]);
                Use::of(self.get_noop_or_sign_extend(cx, operand.scev, width(1)))
            }
            "noopanyext" => {
                let operand = self.eval(cx, &kids[2]);
                Use::of(self.get_noop_or_any_extend(cx, operand.scev, width(1)))
            }
            other => panic!("unknown op {other}"),
        }
    }

    /// One oracle line: the expression, ` : `, its type.
    pub fn eval_line(
        &mut self,
        cx: &Cx,
        labels: &Labels,
        line: &str,
    ) -> String {
        let result = self.eval(cx, &parse(line));
        let _: Scev = result.scev;
        let _ = ValueDef::Argument;
        format!("{} : {}", self.print_use(cx, labels, result), type_text(self.type_of(result.scev)))
    }
}
