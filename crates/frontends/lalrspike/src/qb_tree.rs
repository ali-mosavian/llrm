//! The generic tree the QB grammar builds: rule names over token indexes,
//! with no meaning attached. A later pass gives names, calls and array
//! elements theirs.

use crate::qb_kw::Kw;
use crate::qb_tokens::Tok;

#[derive(Clone, Debug, PartialEq)]
pub enum Tree {
    Empty,
    /// A reserved word or operator.
    Kw(Kw),
    /// A name, number or string: the index of its token.
    At(usize),
    Node(&'static str, Vec<Tree>),
    List(Vec<Tree>),
}

impl Tree {
    pub fn tok(token: Tok) -> Self {
        match token {
            Tok::Kw(kw) => Tree::Kw(kw),
            other => unreachable!("{other:?} is a leaf with a payload"),
        }
    }

    pub fn at_ext(token: Tok) -> Self {
        let _ = token;
        Tree::Empty
    }

    pub fn at(index: usize) -> Self {
        Tree::At(index)
    }

    pub fn node(rule: &'static str, children: Vec<Tree>) -> Self {
        Tree::Node(rule, children)
    }

    pub fn push(list: Tree, item: Tree) -> Self {
        match list {
            Tree::List(mut items) => {
                items.push(item);
                Tree::List(items)
            }
            other => unreachable!("{other:?} is no list"),
        }
    }

    pub fn opt(value: Option<Tree>) -> Self {
        value.unwrap_or(Tree::Empty)
    }

    /// Every node's rule name, in source order.
    pub fn rules<'a>(&'a self, out: &mut Vec<&'static str>) {
        match self {
            Tree::Node(rule, children) => {
                out.push(rule);
                children.iter().for_each(|child| child.rules(out));
            }
            Tree::List(children) => children.iter().for_each(|child| child.rules(out)),
            _ => {}
        }
    }
}
