//! Total decoded source occurrences for one OMF body.
//!
//! Direct port of `qbopt.model.ir.py:{Opaque,Long,Call,Restore,TableKind,
//! Data,Node,RESTORE_EFFECTS,pinned,span}`.  This is frontend OMF data: it
//! retains source-byte spans and must not be projected onto generic MIR.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::LazyLock;

use super::{Effects, RESTORE_IDIOM, Semantics, TABLE_DATA, UNMODELLED, barrier};

use iced_x86::Register;

use crate::frontend::declen::Insn;
use crate::legacy::lift::Decoded;

/// An instruction with no long-pair or call idiom attached to it.
///
/// Direct port of `qbopt.model.ir:Opaque`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Opaque {
    pub insn: Insn,
    pub effects: Effects,
    pub semantics: Semantics,
}

impl Opaque {
    #[must_use]
    pub fn new(insn: Insn, effects: Effects) -> Self {
        Self {
            insn,
            effects,
            semantics: UNMODELLED.clone(),
        }
    }
}

/// One `lift.classify()` long-pair source form.
///
/// Direct port of `qbopt.model.ir:Long`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Long {
    pub insn: Insn,
    pub decoded: Decoded,
    pub effects: Effects,
    pub semantics: Semantics,
}

impl Long {
    #[must_use]
    pub fn new(insn: Insn, decoded: Decoded, effects: Effects) -> Self {
        Self {
            insn,
            decoded,
            effects,
            semantics: UNMODELLED.clone(),
        }
    }
}

/// A far call identified by its OMF fixup name.
///
/// Direct port of `qbopt.model.ir:Call`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Call {
    pub insn: Insn,
    pub name: String,
    pub effects: Effects,
    pub semantics: Semantics,
}

impl Call {
    #[must_use]
    pub fn new(insn: Insn, name: String, effects: Effects) -> Self {
        Self {
            insn,
            name,
            effects,
            semantics: UNMODELLED.clone(),
        }
    }
}

/// The pair-specific effects of `lift::FIXUP`'s restore bytes.
///
/// Direct port of `qbopt.model.ir:RESTORE_EFFECTS`.
pub static RESTORE_EFFECTS: LazyLock<BTreeMap<usize, Effects>> = LazyLock::new(|| {
    BTreeMap::from([
        (0, restore_effects(iced_x86::Register::EAX, iced_x86::Register::EDX)),
        (1, restore_effects(iced_x86::Register::ECX, iced_x86::Register::EBX)),
    ])
});

fn restore_effects(low: iced_x86::Register, high: iced_x86::Register) -> Effects {
    let registers = BTreeSet::from([low, high]);
    Effects {
        defs: Some(registers.clone()),
        uses: Some(registers),
        flags_written: crate::model::ir::Flag::NONE,
        flags_read: crate::model::ir::Flag::NONE,
        loads: Vec::new(),
        stores: Vec::new(),
        fp_stack: false,
        memory_complete: false,
    }
}

/// A three-instruction widened-value restore idiom.
///
/// Direct port of `qbopt.model.ir:Restore`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Restore {
    pub at: usize,
    pub end: usize,
    pub pair: usize,
    pub effects: Effects,
    pub semantics: Semantics,
}

impl Restore {
    #[must_use]
    pub fn new(at: usize, end: usize, pair: usize, effects: Effects) -> Self {
        Self {
            at,
            end,
            pair,
            effects,
            semantics: RESTORE_IDIOM.clone(),
        }
    }
}

/// The purpose of an inline OMF data table.
///
/// Direct port of `qbopt.model.ir:TableKind`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TableKind {
    Jump,
    Map,
}

impl TableKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Jump => "jump",
            Self::Map => "map",
        }
    }
}

impl fmt::Display for TableKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Ord for TableKind {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl PartialOrd for TableKind {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Inline body data that is not an instruction.
///
/// Direct port of `qbopt.model.ir:Data`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Data {
    pub at: usize,
    pub end: usize,
    pub kind: TableKind,
    pub entries: Vec<usize>,
    pub effects: Effects,
    pub semantics: Semantics,
}

impl Data {
    #[must_use]
    pub fn new(
        at: usize,
        end: usize,
        kind: TableKind,
        entries: Vec<usize>,
        effects: Effects,
    ) -> Self {
        Self {
            at,
            end,
            kind,
            entries,
            effects,
            semantics: TABLE_DATA.clone(),
        }
    }
}

/// Every source occurrence in a decoded OMF body.
///
/// Direct port of `qbopt.model.ir:Node`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Node {
    Opaque(Opaque),
    Long(Long),
    Call(Call),
    Restore(Restore),
    Data(Data),
}

impl Node {
    fn semantics(&self) -> &Semantics {
        match self {
            Self::Opaque(node) => &node.semantics,
            Self::Long(node) => &node.semantics,
            Self::Call(node) => &node.semantics,
            Self::Restore(node) => &node.semantics,
            Self::Data(node) => &node.semantics,
        }
    }

    fn effects(&self) -> &Effects {
        match self {
            Self::Opaque(node) => &node.effects,
            Self::Long(node) => &node.effects,
            Self::Call(node) => &node.effects,
            Self::Restore(node) => &node.effects,
            Self::Data(node) => &node.effects,
        }
    }
}

/// Registers a barrier must retain at this source occurrence.
///
/// Direct port of `qbopt.model.ir:pinned`.
#[must_use]
pub fn pinned(node: &Node) -> Option<BTreeSet<iced_x86::Register>> {
    if !barrier(node.semantics()) {
        return Some(BTreeSet::new());
    }
    let effects = node.effects();
    let (Some(defs), Some(uses)) = (&effects.defs, &effects.uses) else {
        return None;
    };
    Some(defs.union(uses).copied().collect())
}

/// The exact source-byte span represented by a node.
///
/// Direct port of `qbopt.model.ir:span`.
#[must_use]
pub fn span(node: &Node) -> (usize, usize) {
    match node {
        Node::Opaque(node) => (node.insn.at, node.insn.end()),
        Node::Long(node) => (node.insn.at, node.insn.end()),
        Node::Call(node) => (node.insn.at, node.insn.end()),
        Node::Restore(node) => (node.at, node.end),
        Node::Data(node) => (node.at, node.end),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        Call, Data, Long, Node, Opaque, RESTORE_EFFECTS, Restore, TableKind, pinned, span,
    };
    use crate::frontend::declen::decode;
    use crate::legacy::lift::{Decoded, Kind};
    use crate::model::ir::{Effects, RESTORE_IDIOM, TABLE_DATA, UNMODELLED, barrier};
    

    fn insn(at: usize) -> crate::frontend::declen::Insn {
        let mut bytes = vec![0x90; at];
        bytes.extend([0x89, 0xC0]);
        decode(&bytes, at).unwrap()
    }

    fn effects() -> Effects {
        Effects::no_effect()
    }

    #[test]
    fn omf_source_nodes_table_kind_spelling_and_order_are_python_strenum_order() {
        let mut kinds = vec![TableKind::Jump, TableKind::Map];
        kinds.sort();
        assert_eq!(
            kinds.into_iter().map(TableKind::as_str).collect::<Vec<_>>(),
            ["jump", "map"]
        );
        assert_eq!(TableKind::Jump.to_string(), "jump");
    }

    #[test]
    fn omf_source_nodes_all_five_constructors_use_their_python_default_semantics() {
        let opaque = Opaque::new(insn(0), effects());
        let long = Long::new(insn(2), Decoded::new(Kind::Load, 0, 0, 2), effects());
        let call = Call::new(insn(4), "B$MUI4".to_owned(), effects());
        let restore = Restore::new(6, 10, 0, RESTORE_EFFECTS.get(&0).unwrap().clone());
        let data = Data::new(10, 16, TableKind::Jump, vec![14, 12], effects());

        assert_eq!(opaque.semantics, *UNMODELLED);
        assert_eq!(long.semantics, *UNMODELLED);
        assert_eq!(call.semantics, *UNMODELLED);
        assert_eq!(restore.semantics, *RESTORE_IDIOM);
        assert_eq!(data.semantics, *TABLE_DATA);
        assert_eq!(data.entries, vec![14, 12]);
    }

    #[test]
    fn omf_source_nodes_restore_effects_name_both_partial_write_roots_and_nothing_else() {
        let pair_zero = RESTORE_EFFECTS.get(&0).unwrap();
        let pair_one = RESTORE_EFFECTS.get(&1).unwrap();
        assert_eq!(
            pair_zero.defs,
            Some(BTreeSet::from([
                iced_x86::Register::EAX,
                iced_x86::Register::EDX
            ]))
        );
        assert_eq!(pair_zero.uses, pair_zero.defs);
        assert_eq!(
            pair_one.defs,
            Some(BTreeSet::from([
                iced_x86::Register::ECX,
                iced_x86::Register::EBX
            ]))
        );
        assert_eq!(pair_one.uses, pair_one.defs);
        for effects in [pair_zero, pair_one] {
            assert_eq!(effects.flags_written.bits(), 0);
            assert_eq!(effects.flags_read.bits(), 0);
            assert!(effects.loads.is_empty());
            assert!(effects.stores.is_empty());
            assert!(!effects.fp_stack);
            assert!(!effects.memory_complete);
        }
    }

    #[test]
    fn omf_source_nodes_pinned_distinguishes_modelled_known_and_unknown_barriers() {
        let modelled = Node::Restore(Restore::new(
            0,
            4,
            0,
            RESTORE_EFFECTS.get(&0).unwrap().clone(),
        ));
        assert_eq!(pinned(&modelled), Some(BTreeSet::new()));

        let mut known_effects = effects();
        known_effects.defs = Some(BTreeSet::from([iced_x86::Register::EAX]));
        known_effects.uses = Some(BTreeSet::from([iced_x86::Register::EDX]));
        let known = Node::Opaque(Opaque::new(insn(4), known_effects));
        assert!(barrier(known.semantics()));
        assert_eq!(
            pinned(&known),
            Some(BTreeSet::from([
                iced_x86::Register::EAX,
                iced_x86::Register::EDX
            ]))
        );

        let mut unknown_effects = effects();
        unknown_effects.defs = None;
        let unknown = Node::Opaque(Opaque::new(insn(6), unknown_effects));
        assert_eq!(pinned(&unknown), None);
    }

    #[test]
    fn omf_source_nodes_span_covers_every_node_variant() {
        let values = [
            (Node::Opaque(Opaque::new(insn(0), effects())), (0, 2)),
            (
                Node::Long(Long::new(
                    insn(2),
                    Decoded::new(Kind::Load, 0, 0, 2),
                    effects(),
                )),
                (2, 4),
            ),
            (
                Node::Call(Call::new(insn(4), "B$MUI4".to_owned(), effects())),
                (4, 6),
            ),
            (
                Node::Restore(Restore::new(
                    6,
                    10,
                    0,
                    RESTORE_EFFECTS.get(&0).unwrap().clone(),
                )),
                (6, 10),
            ),
            (
                Node::Data(Data::new(10, 14, TableKind::Map, vec![12], effects())),
                (10, 14),
            ),
        ];
        for (node, wanted) in values {
            assert_eq!(span(&node), wanted);
        }
    }
}
