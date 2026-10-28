//! A pruned tree to the runtime's flat items.

use std::collections::BTreeMap;

use llrm_dagisel::matcher::{Emit, Matcher, Opcode, Slice, Vt};

use crate::decode::Node;

pub struct Flat {
    pub items: Vec<Matcher>,
    pub operands: Vec<u16>,
}

struct Flattener<'a> {
    lists: &'a [u16],
    items: Vec<Matcher>,
    operands: Vec<u16>,
    seen: BTreeMap<Vec<u16>, u32>,
}

pub fn flatten(
    nodes: &[Node],
    lists: &[u16],
) -> Flat {
    let mut f = Flattener { lists, items: Vec::new(), operands: Vec::new(), seen: BTreeMap::new() };
    f.sequence(nodes);
    Flat { items: f.items, operands: f.operands }
}

impl Flattener<'_> {
    fn slice(
        &mut self,
        old: Slice,
    ) -> Slice {
        let list = self.lists[old.start as usize..old.start as usize + old.len as usize].to_vec();
        let operands = &mut self.operands;
        let start = *self
            .seen
            .entry(list.clone())
            .or_insert_with(
                || {
                    let start = operands.len() as u32;
                    operands.extend_from_slice(&list);
                    start
                },
            );
        Slice { start, len: old.len }
    }

    fn item(
        &mut self,
        item: Matcher,
    ) -> Matcher {
        match item {
            Matcher::EmitMergeInputChains(s) => Matcher::EmitMergeInputChains(self.slice(s)),
            Matcher::CompleteMatch(s) => Matcher::CompleteMatch(self.slice(s)),
            Matcher::EmitNode(e) => Matcher::EmitNode(Emit { operands: self.slice(e.operands), ..e }),
            Matcher::MorphNodeTo(e) => Matcher::MorphNodeTo(Emit { operands: self.slice(e.operands), ..e }),
            other => other,
        }
    }

    fn sequence(
        &mut self,
        nodes: &[Node],
    ) {
        for node in nodes {
            match node {
                Node::Item(item) => {
                    let item = self.item(*item);
                    self.items.push(item);
                }
                Node::Scope(alternatives) => {
                    self.items.push(Matcher::Scope);
                    for alt in alternatives {
                        let header = self.items.len();
                        self.items.push(Matcher::Alt(0));
                        self.sequence(alt);
                        self.items[header] = Matcher::Alt((self.items.len() - header - 1) as u32);
                    }
                    self.items.push(Matcher::Alt(0));
                }
                Node::SwitchOpcode(cases) => {
                    self.items.push(Matcher::SwitchOpcode);
                    for (opcode, body) in cases {
                        self.case(body, |size| Matcher::OpcodeCase { size, opcode: *opcode });
                    }
                    self.items.push(Matcher::OpcodeCase { size: 0, opcode: Opcode::default() });
                }
                Node::SwitchType(cases) => {
                    self.items.push(Matcher::SwitchType);
                    for (vt, body) in cases {
                        self.case(body, |size| Matcher::TypeCase { size, vt: *vt });
                    }
                    self.items.push(Matcher::TypeCase { size: 0, vt: Vt(0) });
                }
            }
        }
    }

    fn case(
        &mut self,
        body: &[Node],
        header: impl Fn(u32) -> Matcher,
    ) {
        let at = self.items.len();
        self.items.push(header(0));
        self.sequence(body);
        self.items[at] = header((self.items.len() - at - 1) as u32);
    }
}
