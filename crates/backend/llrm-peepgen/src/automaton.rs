//! One group's rules as a decision automaton.
//!
//! Every rule is a set of tests on the window: each instruction's operation,
//! mnemonic and operand counts, then each operand's kind, width and
//! immediate. The automaton tests one key per state, in a fixed order, and
//! branches on its value; rules that agree on a prefix of tests share the
//! states that make them (GCC genmatch's decision tree, LLVM GlobalISel's
//! match table). Identical sub-automata are one state. What is left at an
//! accepting state -- equal operands and guards -- the rules there run in
//! their order in the file.

use std::collections::{BTreeMap, BTreeSet};

use llrm_support::hash::HashMap;

use crate::resolve::{RGroup, RInsn, Side, Ty, Var};
use crate::syntax::{Class, OperandPat};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Part {
    Kind,
    Width,
    /// An immediate carries no address.
    Bare,
    Value,
}

/// What a state tests. An instruction's operation, its operand counts and
/// operands come before its mnemonic, so rules differing only in mnemonic
/// share the states that test the rest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Key {
    Op(usize),
    Name(usize),
    Dests(usize),
    Sources(usize),
    Operand(usize, Side, usize, Part),
}

impl Key {
    pub fn slot(self) -> usize {
        match self {
            Key::Op(slot) | Key::Name(slot) | Key::Dests(slot) | Key::Sources(slot) | Key::Operand(slot, ..) => slot,
        }
    }

    fn order(self) -> (usize, u8, Side, usize, Part) {
        match self {
            Key::Op(slot) => (slot, 0, Side::D, 0, Part::Kind),
            Key::Name(slot) => (slot, 5, Side::D, 0, Part::Kind),
            Key::Dests(slot) => (slot, 2, Side::D, 0, Part::Kind),
            Key::Sources(slot) => (slot, 3, Side::D, 0, Part::Kind),
            Key::Operand(slot, side, index, part) => (slot, 4, side, index, part),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Val {
    Op(String),
    Name(String),
    Count(usize),
    Kind(&'static str),
    Width(u32),
    Bool(bool),
    Int(i64),
}

pub type NodeId = usize;

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum Node {
    Fail,
    /// The rules left, in priority order.
    Accept(Vec<usize>),
    Switch {
        key: Key,
        arms: Vec<(Val, NodeId)>,
        default: NodeId,
    },
}

#[derive(Debug)]
pub struct Automaton {
    /// Node 0 is failure.
    pub nodes: Vec<Node>,
    pub root: NodeId,
}

impl Automaton {
    /// States other than failure.
    pub fn states(&self) -> usize {
        self.nodes.len() - 1
    }

    /// The states the automaton is in once the first instruction is tested.
    pub fn boundary(
        &self,
        id: NodeId,
    ) -> bool {
        match &self.nodes[id] {
            Node::Fail => false,
            Node::Accept(_) => true,
            Node::Switch { key, .. } => key.slot() > 0,
        }
    }

    /// The rules any path from `id` accepts, in priority order.
    pub fn reachable(
        &self,
        id: NodeId,
    ) -> BTreeSet<usize> {
        match &self.nodes[id] {
            Node::Fail => BTreeSet::new(),
            Node::Accept(rules) => rules.iter().copied().collect(),
            Node::Switch { arms, default, .. } => {
                let mut out = self.reachable(*default);
                for (_, arm) in arms {
                    out.extend(self.reachable(*arm));
                }
                out
            }
        }
    }
}

type Tests = BTreeMap<Key, BTreeSet<Val>>;

fn kind_of(class: Class) -> Option<&'static str> {
    match class {
        Class::Reg => Some("Reg"),
        Class::Mem => Some("Mem"),
        Class::Imm | Class::Sym => Some("Imm"),
        Class::Held => Some("Held"),
        Class::Any => None,
    }
}

fn ty_kind(ty: Ty) -> Option<&'static str> {
    match ty {
        Ty::Reg => Some("Reg"),
        Ty::Mem => Some("Mem"),
        Ty::Imm => Some("Imm"),
        Ty::Held => Some("Held"),
        Ty::Loc | Ty::Name => None,
    }
}

/// The tests one rule's pattern makes: one set per operation its heads
/// allow, so that a mnemonic is only ever tested under its own operation.
pub fn tests(
    insns: &[RInsn],
    vars: &indexmap::IndexMap<String, Var>,
) -> Vec<Tests> {
    let mut out = Tests::new();
    for (slot, insn) in insns.iter().enumerate() {
        out.extend(operand_tests(insn, slot, vars));
    }
    let mut rows = vec![out];
    for (slot, insn) in insns.iter().enumerate() {
        rows = rows
            .into_iter()
            .flat_map(|row| {
                insn.ops.iter().map(move |op| {
                    let mut row = row.clone();
                    row.insert(Key::Op(slot), BTreeSet::from([Val::Op(op.clone())]));
                    if let Some(names) = &insn.names {
                        let names = names.iter().filter(|(_, of)| of == op).map(|(name, _)| Val::Name(name.clone()));
                        row.insert(Key::Name(slot), names.collect());
                    }
                    row
                })
            })
            .collect();
    }
    rows
}

/// The tests on one instruction's operand counts and operands.
pub fn operand_tests(
    insn: &RInsn,
    slot: usize,
    vars: &indexmap::IndexMap<String, Var>,
) -> Tests {
    let mut out = Tests::new();
    let mut one = |key: Key, value: Val| {
        out.insert(key, BTreeSet::from([value]));
    };
    one(Key::Dests(slot), Val::Count(insn.dests));
    if let Some(sources) = insn.sources {
        one(Key::Sources(slot), Val::Count(sources));
    }
    for (side, index, operand) in &insn.operands {
        let key = |part| Key::Operand(slot, *side, *index, part);
        match operand {
            OperandPat::Wild => {}
            OperandPat::Lit { value, width } => {
                one(key(Part::Kind), Val::Kind("Imm"));
                one(key(Part::Bare), Val::Bool(true));
                one(key(Part::Value), Val::Int(*value));
                if let Some(width) = width {
                    one(key(Part::Width), Val::Width(*width));
                }
            }
            OperandPat::Bind { kind: Some((class, width)), .. } => {
                if let Some(kind) = kind_of(*class) {
                    one(key(Part::Kind), Val::Kind(kind));
                }
                if *class == Class::Imm {
                    one(key(Part::Bare), Val::Bool(true));
                }
                if let Some(width) = width {
                    one(key(Part::Width), Val::Width(*width));
                }
            }
            OperandPat::Bind { name, kind: None } => {
                if let Some(kind) = ty_kind(vars[name].ty) {
                    one(key(Part::Kind), Val::Kind(kind));
                }
            }
        }
    }
    out
}

#[derive(Default)]
struct Builder {
    nodes: Vec<Node>,
    shared: HashMap<Node, NodeId>,
    memo: HashMap<Vec<(usize, Vec<(Key, Vec<Val>)>)>, NodeId>,
}

impl Builder {
    fn add(
        &mut self,
        node: Node,
    ) -> NodeId {
        if let Some(id) = self.shared.get(&node) {
            return *id;
        }
        let id = self.nodes.len();
        let copy = match &node {
            Node::Fail => Node::Fail,
            Node::Accept(rules) => Node::Accept(rules.clone()),
            Node::Switch { key, arms, default } => Node::Switch { key: *key, arms: arms.clone(), default: *default },
        };
        self.nodes.push(node);
        self.shared.insert(copy, id);
        id
    }

    fn build(
        &mut self,
        rows: Vec<(usize, Tests)>,
    ) -> NodeId {
        if rows.is_empty() {
            return 0;
        }
        let signature: Vec<(usize, Vec<(Key, Vec<Val>)>)> = rows
            .iter()
            .map(|(rule, tests)| {
                (*rule, tests.iter().map(|(key, values)| (*key, values.iter().cloned().collect())).collect())
            })
            .collect();
        if let Some(id) = self.memo.get(&signature) {
            return *id;
        }
        let key = rows.iter().flat_map(|(_, tests)| tests.keys().copied()).min_by_key(|key| key.order());
        let id = match key {
            None => {
                let mut rules: Vec<usize> = rows.iter().map(|(rule, _)| *rule).collect();
                rules.dedup();
                self.add(Node::Accept(rules))
            }
            Some(key) => {
                let values: BTreeSet<Val> =
                    rows.iter().filter_map(|(_, tests)| tests.get(&key)).flatten().cloned().collect();
                let taken = |value: Option<&Val>| -> Vec<(usize, Tests)> {
                    rows.iter()
                        .filter(|(_, tests)| match (tests.get(&key), value) {
                            (None, _) => true,
                            (Some(allowed), Some(value)) => allowed.contains(value),
                            (Some(_), None) => false,
                        })
                        .map(|(rule, tests)| {
                            let mut tests = tests.clone();
                            tests.remove(&key);
                            (*rule, tests)
                        })
                        .collect()
                };
                let default = self.build(taken(None));
                let mut arms = Vec::new();
                for value in values {
                    let arm = self.build(taken(Some(&value)));
                    if arm != default {
                        arms.push((value, arm));
                    }
                }
                if arms.is_empty() { default } else { self.add(Node::Switch { key, arms, default }) }
            }
        };
        self.memo.insert(signature, id);
        id
    }
}

pub fn build(group: &RGroup) -> Automaton {
    let mut builder = Builder::default();
    builder.nodes.push(Node::Fail);
    builder.shared.insert(Node::Fail, 0);
    let rows = group
        .rules
        .iter()
        .enumerate()
        .flat_map(|(at, rule)| tests(&rule.insns[..rule.window], &rule.vars).into_iter().map(move |one| (at, one)))
        .collect();
    let root = builder.build(rows);
    Automaton { nodes: builder.nodes, root }
}
