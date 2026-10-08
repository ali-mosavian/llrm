//! The decision automaton: each feature of an instruction tested once, on
//! the path every pattern still standing shares, as TableGen's matcher
//! merges common prefixes. Identical states are one state.

use llrm_support::hash::HashMap;

use super::parse::{Pattern, KINDS, OPCODES, OPERANDS, TYPES};

/// Opcode, result type, then each operand's kind and type.
pub const FEATURES: usize = 2 + 2 * OPERANDS;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum State {
    /// Test `feature`: the state for each value, else `default`.
    Test { feature: usize, edges: Vec<(u16, usize)>, default: Option<usize> },
    /// The patterns that matched, in file order.
    Leaf(Vec<usize>),
}

#[derive(Debug)]
pub struct Automaton {
    pub states: Vec<State>,
    pub root: Option<usize>,
}

fn ids(domain: &[&str], names: &[String]) -> Vec<u16> {
    let mut out: Vec<u16> = names.iter().map(|name| domain.iter().position(|one| one == name).expect("a checked name") as u16).collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// The values `pattern` accepts at each feature; None accepts any.
pub fn constraints(pattern: &Pattern) -> Vec<Option<Vec<u16>>> {
    let mut out = vec![pattern.opcodes.as_ref().map(|one| ids(&OPCODES, one)), pattern.result.as_ref().map(|one| ids(&TYPES, one))];
    // An operand a pattern lists is there.
    let present: Vec<String> = KINDS.iter().filter(|&&one| one != "none").map(|one| one.to_string()).collect();
    for index in 0..OPERANDS {
        let operand = pattern.operands.get(index);
        out.push(operand.map(|one| ids(&KINDS, one.kinds.as_ref().unwrap_or(&present))));
        out.push(operand.and_then(|one| one.types.as_ref()).map(|one| ids(&TYPES, one)));
    }
    out
}

struct Builder<'a> {
    constraints: &'a [Vec<Option<Vec<u16>>>],
    states: Vec<State>,
    made: HashMap<State, usize>,
}

impl Builder<'_> {
    fn intern(&mut self, state: State) -> usize {
        if let Some(&index) = self.made.get(&state) {
            return index;
        }
        self.states.push(state.clone());
        self.made.insert(state, self.states.len() - 1);
        self.states.len() - 1
    }

    fn node(&mut self, standing: Vec<usize>, feature: usize) -> Option<usize> {
        if standing.is_empty() {
            return None;
        }
        if feature == FEATURES {
            return Some(self.intern(State::Leaf(standing)));
        }
        let mut values: Vec<u16> = standing.iter().filter_map(|&one| self.constraints[one][feature].clone()).flatten().collect();
        if values.is_empty() {
            return self.node(standing, feature + 1);
        }
        values.sort_unstable();
        values.dedup();
        let accepts = |pattern: usize, value: Option<u16>| match (&self.constraints[pattern][feature], value) {
            (None, _) => true,
            (Some(set), Some(value)) => set.contains(&value),
            (Some(_), None) => false,
        };
        let children: Vec<(u16, Vec<usize>)> = values.iter().map(|&value| (value, standing.iter().copied().filter(|&one| accepts(one, Some(value))).collect())).collect();
        let others: Vec<usize> = standing.iter().copied().filter(|&one| accepts(one, None)).collect();
        let mut edges = Vec::new();
        for (value, child) in children {
            if let Some(next) = self.node(child, feature + 1) {
                edges.push((value, next));
            }
        }
        let default = self.node(others, feature + 1);
        edges.retain(|&(_, next)| Some(next) != default);
        if edges.is_empty() {
            return default;
        }
        Some(self.intern(State::Test { feature, edges, default }))
    }
}

pub fn build(patterns: &[Pattern]) -> Automaton {
    let constraints: Vec<_> = patterns.iter().map(constraints).collect();
    let mut builder = Builder { constraints: &constraints, states: Vec::new(), made: HashMap::default() };
    let root = builder.node((0..patterns.len()).collect(), 0);
    Automaton { states: builder.states, root }
}
