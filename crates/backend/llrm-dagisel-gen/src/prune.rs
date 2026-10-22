//! Drops what the CPU cannot run from the decoded table.
//!
//! A check that is false for the CPU kills the sequence it is in; a check that
//! is true is dropped. Sequences are alternatives of a `Scope`, cases of a
//! switch, or the whole table, so a dead alternative disappears and a `Scope`
//! left with one alternative is inlined.

use std::collections::{BTreeMap, BTreeSet};

use llrm_dagisel::matcher::{Matcher, Vt};

use crate::decode::Node;
use crate::features::Cpu;
use crate::inc::Ids;
use crate::predicate::{self, Decided, Expr};

/// What pruning kept of the pattern predicates: the simplified expression of
/// each one still checked at run time.
pub type Open = BTreeMap<u16, Expr>;

pub struct Pruner {
    decided: Vec<Decided>,
    dead_vts: BTreeSet<u16>,
    pub open: Open,
}

impl Pruner {
    pub fn new(
        cpu: &Cpu,
        predicates: &[String],
        ids: &Ids,
    ) -> Result<Pruner, String> {
        let decided = predicates
            .iter()
            .enumerate()
            .map(|(i, text)| {
                predicate::parse(text)
                    .and_then(|e| e.decide(&|atom| cpu.verdict(atom)))
                    .map_err(|e| format!("pattern predicate {i}: {e}"))
            })
            .collect::<Result<_, _>>()?;
        let dead_vts = ids
            .0
            .iter()
            .filter_map(|(name, &value)| name.strip_prefix("MVT::").filter(|n| cpu.dead_type(n)).map(|_| value as u16))
            .collect();
        Ok(Pruner { decided, dead_vts, open: Open::new() })
    }

    pub fn prune(
        &mut self,
        nodes: &[Node],
    ) -> Vec<Node> {
        self.sequence(nodes).unwrap_or_default()
    }

    /// `None` when the sequence can never match.
    fn sequence(
        &mut self,
        nodes: &[Node],
    ) -> Option<Vec<Node>> {
        let mut out = Vec::with_capacity(nodes.len());
        for node in nodes {
            match node {
                Node::Item(item) => {
                    if let Matcher::CheckPatternPredicate(id) = *item {
                        match &self.decided[usize::from(id)] {
                            Decided::No => return None,
                            Decided::Yes => continue,
                            Decided::Open(expr) => {
                                self.open.insert(id, expr.clone());
                            }
                        }
                    }
                    if self.dead_item(item) {
                        return None;
                    }
                    out.push(node.clone());
                }
                Node::Scope(alternatives) => {
                    let mut live: Vec<Vec<Node>> = alternatives.iter().filter_map(|alt| self.sequence(alt)).collect();
                    match live.len() {
                        0 => return None,
                        1 => out.extend(live.pop().unwrap()),
                        _ => out.push(Node::Scope(live)),
                    }
                }
                Node::SwitchOpcode(cases) => {
                    let live: Vec<_> =
                        cases.iter().filter_map(|(op, body)| self.sequence(body).map(|b| (*op, b))).collect();
                    if live.is_empty() {
                        return None;
                    }
                    out.push(Node::SwitchOpcode(live));
                }
                Node::SwitchType(cases) => {
                    let live: Vec<_> = cases
                        .iter()
                        .filter(|(vt, _)| !self.dead_vts.contains(&vt.0))
                        .collect::<Vec<_>>()
                        .into_iter()
                        .filter_map(|(vt, body)| self.sequence(body).map(|b| (*vt, b)))
                        .collect();
                    if live.is_empty() {
                        return None;
                    }
                    out.push(Node::SwitchType(live));
                }
            }
        }
        Some(out)
    }

    fn dead_item(
        &self,
        item: &Matcher,
    ) -> bool {
        let dead = |vt: &Vt| self.dead_vts.contains(&vt.0);
        match item {
            Matcher::CheckType(vt) | Matcher::CheckChildType { vt, .. } | Matcher::CheckValueType(vt) => dead(vt),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inc::Ids;

    const FEATURES: &str = include_str!("../features.toml");

    fn pruner(predicates: &[&str]) -> Pruner {
        let cpu = Cpu::load(FEATURES, "i486").unwrap();
        let predicates: Vec<String> = predicates.iter().map(|p| p.to_string()).collect();
        Pruner::new(&cpu, &predicates, &Ids::parse("MVT::i32 7\nMVT::v4i32 75\n").unwrap()).unwrap()
    }

    fn item(m: Matcher) -> Node {
        Node::Item(m)
    }

    #[test]
    fn a_cmov_alternative_is_dropped_and_the_scope_with_one_left_is_inlined() {
        // [needs CMOV: CompleteMatch] | [CheckOpcode, CompleteMatch]
        let cmov = vec![item(Matcher::CheckPatternPredicate(0)), item(Matcher::MoveParent)];
        let plain = vec![item(Matcher::RecordNode), item(Matcher::MoveParent)];
        let mut p = pruner(&["Subtarget->canUseCMOV()"]);
        let kept = p.prune(&[Node::Scope(vec![cmov, plain])]);
        assert_eq!(kept, [item(Matcher::RecordNode), item(Matcher::MoveParent)]);
        assert!(p.open.is_empty());
    }

    #[test]
    fn a_vector_type_case_is_dropped() {
        let mut p = pruner(&[]);
        let body = vec![item(Matcher::RecordNode)];
        let kept = p.prune(&[Node::SwitchType(vec![(Vt(75), body.clone()), (Vt(7), body.clone())])]);
        assert_eq!(kept, [Node::SwitchType(vec![(Vt(7), body)])]);
    }

    #[test]
    fn a_run_time_predicate_stays_and_is_listed() {
        let mut p = pruner(&["shouldOptForSize(MF)"]);
        let kept = p.prune(&[item(Matcher::CheckPatternPredicate(0)), item(Matcher::RecordNode)]);
        assert_eq!(kept.len(), 2);
        assert!(p.open.contains_key(&0));
    }
}
