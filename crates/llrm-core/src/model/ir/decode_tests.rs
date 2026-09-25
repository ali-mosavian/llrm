//! Port of the decode half of `tests/test_ir.py`.

use std::collections::BTreeSet;
use std::path::PathBuf;

use iced_x86::{Code, Register};

use super::*;
use crate::frontends::bc::blocks::Ends;
use crate::frontends::bc::declen::decode;
use crate::frontends::bc::extent::BodyKind;
use crate::model::ir::nodes::pinned;
use crate::model::ir::{ANY_MEMORY, Flag, barrier, modelled, root};
use crate::objectfile::module::tests::{bare, fixtures, loaded, objects};

/// helpers' `hx`.
fn hx(s: &str) -> Vec<u8> {
    let digits: String = s.split_whitespace().collect();
    (0..digits.len()).step_by(2).map(|at| u8::from_str_radix(&digits[at..at + 2], 16).unwrap()).collect()
}

const OPERATOR_OBJECTS: [&str; 4] = ["pds-g2.obj", "qb45.obj", "vbdos-g2.obj", "vbdos-g3.obj"];

fn _decode(source: &PathBuf) -> (Module, Vec<BodyIR>) {
    let found = loaded(source).unwrap();
    let result = decode_module(&found).unwrap_or_else(|refused| panic!("{}: {refused}", source.display()));
    (found, result)
}

fn original(found: &Module, body: &Body) -> Vec<u8> {
    body.ranges.iter().flat_map(|&(lo, hi)| found.code[lo..hi].iter().copied()).collect()
}

fn insns_of(code: &[u8]) -> Vec<Insn> {
    let mut insns = Vec::new();
    let mut at = 0;
    while at < code.len() {
        let insn = decode(code, at).unwrap();
        at = insn.end();
        insns.push(insn);
    }
    insns
}

/// `module.Module([], 0, "T", code, 0, len(code))`.
fn hand_built(code: &[u8]) -> Module {
    let any = loaded(fixtures().join("qb45.obj")).unwrap();
    let mut found = bare(&any, code.to_vec(), 0, code.len() as i64);
    found.records = Vec::new();
    found.seg = 0;
    found.name = "T".to_owned();
    found
}

#[test]
fn test_every_mapped_fixture_round_trips_byte_identical() {
    for path in objects() {
        let (found, bodies) = _decode(&path);
        for body_ir in &bodies {
            assert_eq!(emit(&found, &body_ir.nodes), original(&found, &body_ir.body), "{}", path.display());
        }
    }
}

#[test]
fn test_nodes_tile_every_range_with_no_gap_or_overlap() {
    for path in objects() {
        let (_found, bodies) = _decode(&path);
        for body_ir in &bodies {
            let spans: Vec<(usize, usize)> = body_ir.nodes.iter().map(|node| span(node)).collect();
            let mut index = 0;
            for &(lo, hi) in &body_ir.body.ranges {
                let mut at = lo;
                while at < hi {
                    assert!(index < spans.len(), "{}: ran out of nodes before {hi:#x}", path.display());
                    let (node_lo, node_hi) = spans[index];
                    assert_eq!(node_lo, at, "{}: gap or overlap at {at:#x}", path.display());
                    at = node_hi;
                    index += 1;
                }
            }
            assert_eq!(index, spans.len(), "{}: nodes left over past the body's own ranges", path.display());
        }
    }
}

#[test]
fn test_decode_is_deterministic() {
    fn signature(bodies: &[BodyIR]) -> Vec<(&'static str, usize, usize)> {
        bodies
            .iter()
            .flat_map(|body_ir| body_ir.nodes.iter())
            .map(|node| {
                let kind = match node.as_ref() {
                    Node::Opaque(_) => "Opaque",
                    Node::Long(_) => "Long",
                    Node::Call(_) => "Call",
                    Node::Restore(_) => "Restore",
                    Node::Data(_) => "Data",
                };
                let (lo, hi) = span(node);
                (kind, lo, hi)
            })
            .collect()
    }
    for path in objects() {
        let (_found_a, bodies_a) = _decode(&path);
        let (_found_b, bodies_b) = _decode(&path);
        assert_eq!(signature(&bodies_a), signature(&bodies_b), "{}", path.display());
    }
}

#[test]
fn test_a_wrong_node_type_cannot_corrupt_emitted_bytes() {
    for path in objects() {
        let (found, bodies) = _decode(&path);
        for body_ir in &bodies {
            let reclassified: Vec<Arc<Node>> = body_ir
                .nodes
                .iter()
                .map(|node| match node.as_ref() {
                    Node::Long(Long { insn, effects, .. }) | Node::Call(Call { insn, effects, .. }) => {
                        Arc::new(Node::Opaque(Opaque::new(insn.clone(), effects.clone())))
                    }
                    _ => Arc::clone(node),
                })
                .collect();
            assert_eq!(emit(&found, &reclassified), original(&found, &body_ir.body), "{}", path.display());
        }
    }
}

#[test]
fn test_every_named_call_becomes_a_call_node_and_no_others_do() {
    for name in OPERATOR_OBJECTS {
        let (found, bodies) = _decode(&fixtures().join(name));
        let call_nodes: BTreeSet<i64> = bodies
            .iter()
            .flat_map(|body_ir| body_ir.nodes.iter())
            .filter_map(|node| match node.as_ref() {
                Node::Call(call) => Some(call.insn.at as i64),
                _ => None,
            })
            .collect();
        assert!(!call_nodes.is_empty(), "an operator fixture has at least one runtime call");
        assert_eq!(call_nodes, found.calls.keys().copied().collect::<BTreeSet<i64>>(), "{name}");
        assert!(call_nodes.iter().any(|at| ["B$CPI4", "B$DVI4"].contains(&found.calls[at].as_str())), "{name}");
    }
}

#[test]
fn test_jump_table_is_recognised_by_kind() {
    let (_found, bodies) = _decode(&fixtures().join("jumptable.obj"));
    let tables: Vec<&Data> = bodies
        .iter()
        .flat_map(|body_ir| body_ir.nodes.iter())
        .filter_map(|node| match node.as_ref() {
            Node::Data(data) if data.kind == TableKind::Jump => Some(data),
            _ => None,
        })
        .collect();
    assert!(!tables.is_empty(), "jumptable.obj has an ON GOTO table");
    assert!(tables.iter().all(|node| node.entries.len() == (node.end - node.at - 1) / 2));
}

#[test]
fn test_resume_map_is_recognised_as_data_not_a_jump_table() {
    let (_found, bodies) = _decode(&fixtures().join("divmod-v-g3.obj"));
    let tables: Vec<&Data> = bodies
        .iter()
        .flat_map(|body_ir| body_ir.nodes.iter())
        .filter_map(|node| match node.as_ref() {
            Node::Data(data) => Some(data),
            _ => None,
        })
        .collect();
    assert!(!tables.is_empty());
    assert!(tables.iter().all(|node| node.kind == TableKind::Map));
    assert!(tables.iter().all(|node| node.entries.first() == Some(&node.at)));
}

#[test]
fn test_every_data_node_is_a_table_span() {
    for path in objects() {
        let (found, bodies) = _decode(&path);
        let mapped = code_map(&found).unwrap();
        let table_spans: BTreeSet<(usize, usize)> = mapped.tables.iter().copied().collect();
        for body_ir in &bodies {
            for node in &body_ir.nodes {
                if let Node::Data(data) = node.as_ref() {
                    assert!(table_spans.contains(&(data.at, data.end)), "{}", path.display());
                }
            }
        }
    }
}

#[test]
fn test_restore_idiom_is_recognised_not_split_into_opaques() {
    let code = [hx("90"), hx("66 50 58 5A"), hx("90")].concat();
    let insns = insns_of(&code);
    let block = Block { at: 0, end: code.len(), insns: insns.clone(), ends: Ends::FallsThrough, succ: Vec::new() };
    let found = hand_built(&code);
    let mapped = CodeMap {
        starts: insns.iter().map(|insn| insn.at).collect(),
        leaders: BTreeSet::from([0]),
        ..CodeMap::default()
    };
    let body = Body { kind: BodyKind::Main, seed: 0, name: None, ranges: vec![(0, code.len())] };

    let nodes = decode_body(&found, &mapped, &[block], &body);
    let kinds: Vec<&str> = nodes
        .iter()
        .map(|node| match node.as_ref() {
            Node::Opaque(_) => "Opaque",
            Node::Restore(_) => "Restore",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds, ["Opaque", "Restore", "Opaque"]);
    let Node::Restore(restore) = nodes[1].as_ref() else {
        panic!("a restore");
    };
    assert_eq!(restore.pair, 0);
    assert_eq!((restore.at, restore.end), (1, 5));
    assert_eq!(restore.effects.defs, Some(BTreeSet::from([Register::EAX, Register::EDX])));
    assert_eq!(restore.effects.uses, Some(BTreeSet::from([Register::EAX, Register::EDX])));
    assert_eq!(restore.effects.flags_written, Flag::NONE);
    assert_eq!(emit(&found, &nodes), code);
}

#[test]
fn test_semantics_never_claims_a_register_or_cell_the_effects_do_not() {
    for path in objects() {
        let (_found, bodies) = _decode(&path);
        for body_ir in &bodies {
            for node in &body_ir.nodes {
                let (semantics, effects) = (node.semantics(), node.effects());
                if !modelled(semantics) {
                    continue;
                }
                for where_ in &semantics.dests {
                    if let (Loc::Reg(one), Some(defs)) = (where_, &effects.defs) {
                        assert!(defs.contains(&root(one.register)), "{semantics:?}");
                    }
                    if let Loc::Mem(one) = where_ {
                        assert!(effects.stores.contains(one), "{semantics:?}");
                    }
                }
                for where_ in &semantics.sources {
                    if let (Loc::Reg(one), Some(uses)) = (where_, &effects.uses) {
                        assert!(uses.contains(&root(one.register)), "{semantics:?}");
                    }
                    if let Loc::Mem(one) = where_ {
                        assert!(effects.loads.contains(one), "{semantics:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn test_the_corpus_is_modelled_except_for_exactly_the_refused_encodings() {
    let refused = BTreeSet::from([Code::Movsw_m16_m16]);
    let mut unmodelled = BTreeSet::new();
    for path in objects() {
        let found = loaded(&path).unwrap();
        let Ok(result) = decode_module(&found) else {
            continue;
        };
        for body_ir in &result {
            for node in &body_ir.nodes {
                let insn = match node.as_ref() {
                    Node::Opaque(Opaque { insn, .. }) | Node::Long(Long { insn, .. }) | Node::Call(Call { insn, .. }) => insn,
                    _ => continue,
                };
                if !modelled(node.semantics()) {
                    unmodelled.insert(insn.code());
                }
            }
        }
    }
    assert_eq!(unmodelled, refused);
}

#[test]
fn test_every_body_of_every_kind_is_fully_modelled() {
    let mut liftable: IndexMap<BodyKind, usize> = IndexMap::default();
    let mut refused: IndexMap<BodyKind, usize> = IndexMap::default();
    for path in objects() {
        let found = loaded(&path).unwrap();
        let Ok(result) = decode_module(&found) else {
            continue;
        };
        for body_ir in &result {
            let counted = if body_ir.nodes.iter().all(|node| modelled(node.semantics())) {
                &mut liftable
            } else {
                &mut refused
            };
            *counted.entry(body_ir.body.kind).or_default() += 1;
        }
    }
    assert!(liftable.get(&BodyKind::Main).copied().unwrap_or(0) > 0);
    assert!(refused.get(&BodyKind::Main).copied().unwrap_or(0) > 0);
    assert!(refused.iter().all(|(kind, count)| *kind == BodyKind::Main || *count == 0));
}

#[test]
fn test_a_barrier_is_carried_rather_than_refusing_the_body_it_sits_in() {
    let code = [hx("B8 01 00"), hx("E4 40"), hx("C3")].concat();
    let insns = insns_of(&code);
    let block = Block { at: 0, end: code.len(), insns: insns.clone(), ends: Ends::Return, succ: Vec::new() };
    let found = hand_built(&code);
    let mapped = CodeMap { starts: insns.iter().map(|insn| insn.at).collect(), ..CodeMap::default() };
    let body = Body { kind: BodyKind::Main, seed: 0, name: None, ranges: vec![(0, code.len())] };
    let nodes = decode_body(&found, &mapped, &[block], &body);

    let port = &nodes[1];
    assert!(barrier(port.semantics()));
    assert!(!modelled(port.semantics()));
    assert_eq!(nodes.iter().map(|node| modelled(node.semantics())).collect::<Vec<_>>(), [true, false, true]);
    assert_eq!(emit(&found, &nodes), code);
    assert_eq!(pinned(port), Some(BTreeSet::from([Register::EAX])));
    assert_eq!(pinned(&nodes[0]), Some(BTreeSet::new()));
    assert_eq!(port.effects().loads, *ANY_MEMORY);
    assert_eq!(port.effects().stores, *ANY_MEMORY);
}
