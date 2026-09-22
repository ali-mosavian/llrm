//! What each lowered operation requires of a register.
//!
//! Direct port of `qbopt.model.lir`.  This is deliberately not the newer
//! generic Machine IR: Python LIR owns source-byte spans and retains the
//! source-backed MIR operation which selection, layout, and OMF emission
//! still consult.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::model::ir::nodes::Node;
use crate::model::ir::{Held, Operation, Semantics};

use indexmap::IndexMap;

use super::mir::{self, Arg, Kind, Op};
use crate::support::pyrepr::{self, Repr};

/// One machine instruction, as the thing that emits it needs it.
///
/// Direct port of `qbopt.model.lir:Insn`.  `op` and `node` use shared owned
/// references so cloned LIR instructions retain the same source-backed MIR
/// operation and decoded source occurrence, as Python's frozen dataclass
/// copies do.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Insn {
    pub at: i64,
    pub covers: Option<(i64, i64)>,
    pub what: Option<Semantics>,
    pub defines: Vec<u32>,
    pub uses: Vec<u32>,
    pub clobbers: BTreeSet<iced_x86::Register>,
    pub clobbers_high: BTreeSet<iced_x86::Register>,
    pub spread: Vec<(i64, i64)>,
    pub op: Option<Arc<Op>>,
    pub node: Option<Arc<Node>>,
    pub group: Option<i64>,
    pub requires: Vec<(Held, iced_x86::Register)>,
    pub delivers: Vec<(Held, iced_x86::Register)>,
    pub widths: Vec<(u32, u32)>,
    pub symbol: Option<bool>,
    pub spill_reload: bool,
    pub spill_store: bool,
    pub frame_adjust: bool,
    pub rematerialized: bool,
}

impl Insn {
    /// Constructs Python's five-required-field `Insn` form with every later
    /// field at its dataclass default.
    #[must_use]
    pub fn new(
        at: i64,
        covers: Option<(i64, i64)>,
        what: Option<Semantics>,
        defines: Vec<u32>,
        uses: Vec<u32>,
    ) -> Self {
        Self {
            at,
            covers,
            what,
            defines,
            uses,
            clobbers: BTreeSet::new(),
            clobbers_high: BTreeSet::new(),
            spread: Vec::new(),
            op: None,
            node: None,
            group: None,
            requires: Vec::new(),
            delivers: Vec::new(),
            widths: Vec::new(),
            symbol: None,
            spill_reload: false,
            spill_store: false,
            frame_adjust: false,
            rematerialized: false,
        }
    }

    /// Python `Insn.inserted`.
    #[must_use]
    pub fn inserted(&self) -> bool {
        let idiom = self.op.is_some()
            && matches!(self.node.as_deref(), Some(Node::Restore(_)))
            && self
                .what
                .as_ref()
                .is_some_and(|what| what.op == Operation::Restore);
        !idiom && self.covers.is_some_and(|(start, end)| start == end)
    }

    /// Python `Insn.source`.
    #[must_use]
    pub fn source(&self) -> Option<&Op> {
        (!self.inserted()).then_some(())?;
        self.op.as_deref()
    }

    /// Python `Insn.id`.
    #[must_use]
    pub fn id(&self) -> Option<u32> {
        if self.inserted() && self.symbol != Some(true) {
            return None;
        }
        self.op.as_ref().and_then(|op| op.id)
    }

    /// Python `Insn.kind`.
    #[must_use]
    pub fn kind(&self) -> Kind {
        let Some(source) = self.source() else {
            return self
                .what
                .as_ref()
                .map_or(Kind::Nothing, |what| mir::kind_of(what, &[], &[]));
        };
        if source.kind == Kind::Divmod {
            if let Some(what) = &self.what {
                return mir::kind_of(what, &[], &[]);
            }
        }
        source.kind
    }

    /// Python `Insn.name`.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        if let Some(source) = self.source() {
            return Some(source.name.as_str());
        }
        match &self.what {
            Some(what) => what.name.as_deref(),
            None => Some(""),
        }
    }

    /// Python `Insn.args`.
    #[must_use]
    pub fn args(&self) -> &[Arg] {
        self.source().map_or(&[], |source| &source.args)
    }

    /// Python `Insn.results`.
    #[must_use]
    pub fn results(&self) -> &[Arg] {
        self.source().map_or(&[], |source| &source.results)
    }

    /// Python `Insn.raised`.
    #[must_use]
    pub fn raised(&self) -> Option<&(Vec<Arg>, Vec<Arg>)> {
        self.source().and_then(|source| source.raised.as_ref())
    }

    /// Python `Insn.extra_covers`.
    #[must_use]
    pub fn extra_covers(&self) -> Vec<(i64, i64)> {
        self.spread
            .iter()
            .copied()
            .filter(|span| Some(*span) != self.covers)
            .collect()
    }

    /// Python `Insn.rewritten`.
    #[must_use]
    pub fn rewritten(&self) -> bool {
        self.source()
            .is_none_or(|source| !source.source_backed || mir::rewritten(source))
    }
}

/// One value that is two definitions above this block, by id.
///
/// Direct port of `qbopt.model.lir:Phi`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Phi {
    pub result: u32,
    pub incoming: Vec<(i64, u32)>,
}

impl Repr for Phi {
    fn repr(&self) -> String {
        pyrepr::dataclass(
            "Phi",
            &[("result", self.result.repr()), ("incoming", pyrepr::tuple(&self.incoming))],
        )
    }
}

/// One LIR CFG block.
///
/// Direct port of `qbopt.model.lir:LirBlock`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LirBlock {
    pub at: i64,
    pub insns: Vec<Arc<Insn>>,
    pub succ: Vec<i64>,
    pub phis: Vec<Phi>,
    // See mir.MirBlock.cold.
    pub cold: bool,
}

impl LirBlock {
    #[must_use]
    pub fn new(at: i64, insns: Vec<Arc<Insn>>) -> Self {
        Self {
            at,
            insns,
            succ: Vec::new(),
            phis: Vec::new(),
            cold: false,
        }
    }

    /// Python `LirBlock.arrives`.
    #[must_use]
    pub fn arrives(&self) -> Vec<u32> {
        self.phis.iter().map(|one| one.result).collect()
    }
}

/// One lowered procedure.  Blocks remain in emitted order.
///
/// Direct port of `qbopt.model.lir:LirBody`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LirBody {
    pub name: String,
    pub entry: i64,
    pub blocks: Vec<LirBlock>,
    pub origin: IndexMap<u32, iced_x86::Register>,
    pub pins: IndexMap<u32, iced_x86::Register>,
    pub inputs: BTreeSet<u32>,
    pub loop_trip_counts: Vec<(i64, i64)>,
    pub ordered: bool,
    pub noreturn: bool,
}

impl LirBody {
    /// Constructs Python's five-required-field `LirBody` form.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        entry: i64,
        blocks: Vec<LirBlock>,
        origin: IndexMap<u32, iced_x86::Register>,
        pins: IndexMap<u32, iced_x86::Register>,
    ) -> Self {
        Self {
            name: name.into(),
            entry,
            blocks,
            origin,
            pins,
            inputs: BTreeSet::new(),
            loop_trip_counts: Vec::new(),
            ordered: false,
            noreturn: false,
        }
    }

    /// Python `LirBody.insns`.
    #[must_use]
    pub fn insns(&self) -> Vec<Arc<Insn>> {
        self.blocks
            .iter()
            .flat_map(|block| block.insns.iter().cloned())
            .collect()
    }
}

/// Keep virtual dataflow and source-byte ownership for an elided machine op.
///
/// Direct port of `qbopt.model.lir:anchor`.
#[must_use]
pub fn anchor(one: Arc<Insn>) -> Arc<Insn> {
    let mut anchored = (*one).clone();
    anchored.what = Some(Semantics {
        op: Operation::Nothing,
        name: Some(String::new()),
        dests: Vec::new(),
        sources: Vec::new(),
        target: None,
        indirect: false,
    });
    anchored.clobbers.clear();
    anchored.clobbers_high.clear();
    anchored.group = None;
    anchored.requires.clear();
    anchored.delivers.clear();
    anchored.symbol = Some(false);
    anchored.spill_reload = false;
    anchored.spill_store = false;
    anchored.frame_adjust = false;
    anchored.rematerialized = false;
    Arc::new(anchored)
}

/// `insns` without the ones `drop` picks, their bytes given to a survivor.
///
/// Direct port of `qbopt.model.lir:without`.  The retained operation is the
/// rewritten occurrence, while ownership decisions are deliberately made
/// against the original occurrence, matching Python's two-phase loop.
#[must_use]
pub fn without<F, R>(insns: &[Arc<Insn>], drop: F, rewrite: Option<R>) -> Vec<Arc<Insn>>
where
    F: Fn(&Arc<Insn>) -> bool,
    R: Fn(&Arc<Insn>) -> Arc<Insn>,
{
    let mut out = Vec::new();
    for one in insns {
        let kept = rewrite
            .as_ref()
            .map_or_else(|| Arc::clone(one), |rewrite| rewrite(one));
        if !drop(&kept) {
            out.push(kept);
            continue;
        }
        let Some((start, end)) = one.covers else {
            continue;
        };
        if start == end {
            continue;
        }
        if one.spread.len() > 1 {
            out.push(kept);
            continue;
        }
        let where_ = out.iter().rposition(|previous| {
            previous
                .covers
                .is_some_and(|(previous_start, previous_end)| previous_start != previous_end)
        });
        let Some(where_) = where_ else {
            out.push(kept);
            continue;
        };
        if out[where_]
            .covers
            .is_none_or(|(_, previous_end)| previous_end != start)
        {
            out.push(kept);
            continue;
        }
        let mut replacement = (*out[where_]).clone();
        replacement.covers = Some((replacement.covers.expect("checked above").0, end));
        out[where_] = Arc::new(replacement);
    }
    if out.len() > 1 {
        let following = out.iter().enumerate().skip(1).find_map(|(index, one)| {
            one.covers
                .is_some_and(|(start, end)| start < end)
                .then_some(index)
        });
        let first = Arc::clone(&out[0]);
        let second = following.map(|index| Arc::clone(&out[index]));
        if drop(&first)
            && first.covers.is_some()
            && second.is_some()
            && first.spread.len() <= 1
            && second.as_ref().is_some_and(|second| second.spread.len() <= 1)
            && first.covers.map(|covers| covers.1) == second.as_ref().and_then(|second| second.covers).map(|covers| covers.0)
        {
            let following = following.expect("second is not None");
            let mut replacement = (*out[following]).clone();
            replacement.covers = Some((first.covers.expect("checked").0, out[following].covers.expect("checked").1));
            out[following] = Arc::new(replacement);
            out.remove(0);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{Insn, anchor, without};
    use crate::model::ir::nodes::{Node, Restore};
    use crate::model::ir::{Effects, Held, Operation, Semantics};
    use crate::model::mir::{Kind, Op, OpCode};

    fn instruction(at: i64, covers: Option<(i64, i64)>) -> Arc<Insn> {
        Arc::new(Insn::new(
            at,
            covers,
            Some(Semantics::new(Operation::Move)),
            Vec::new(),
            Vec::new(),
        ))
    }

    #[test]
    fn direct_lir_model_anchor_keeps_virtual_definitions_and_source_ownership() {
        // Port of tests/test_peephole.py::test_fusion_preserves_a_virtual_dataflow_anchor:
        // folding physical code must retain both virtual definitions.
        let mut source = (*instruction(0x10, Some((0x10, 0x12)))).clone();
        source.defines = vec![2];
        source.uses = vec![1];
        source.group = Some(7);
        source.requires = vec![(
            Held { value: 1, width: 2 },
            iced_x86::Register::DL,
        )];
        source.spill_reload = true;
        let source = Arc::new(source);
        let anchored = anchor(Arc::clone(&source));
        assert!(!Arc::ptr_eq(&anchored, &source));
        assert_eq!(anchored.defines, vec![2]);
        assert_eq!(anchored.uses, vec![1]);
        assert_eq!(anchored.covers, Some((0x10, 0x12)));
        assert_eq!(anchored.what.as_ref().unwrap().op, Operation::Nothing);
        assert!(anchored.clobbers.is_empty());
        assert!(anchored.requires.is_empty());
        assert_eq!(anchored.symbol, Some(false));
        assert!(!anchored.spill_reload);
    }

    #[test]
    fn direct_lir_model_preserves_python_provenance_properties() {
        // Port of qbopt.model.lir:Insn.{inserted,source,id,kind,name,args,
        // results,raised,extra_covers,rewritten}. The restore exception is
        // source-owned despite its zero-width primary span.
        let mut source = Op::new(
            0x10,
            Some(OpCode::Operation(Operation::Divide)),
            "source-divmod",
            Vec::new(),
            Vec::new(),
        );
        source.id = Some(44);
        source.kind = Kind::Divmod;
        source.source_backed = true;
        source.raised = Some((Vec::new(), Vec::new()));

        let mut lowered = (*instruction(0x10, Some((0x10, 0x12)))).clone();
        lowered.what = Some(Semantics::new(Operation::Divide));
        lowered.op = Some(Arc::new(source));
        lowered.spread = vec![(0x10, 0x12), (0x20, 0x22)];
        assert!(!lowered.inserted());
        assert_eq!(lowered.source().unwrap().id, Some(44));
        assert_eq!(lowered.id(), Some(44));
        assert_eq!(lowered.kind(), Kind::Div);
        assert_eq!(lowered.name(), Some("source-divmod"));
        assert!(lowered.args().is_empty());
        assert!(lowered.results().is_empty());
        assert!(lowered.raised().is_some());
        assert_eq!(lowered.extra_covers(), vec![(0x20, 0x22)]);
        assert!(!lowered.rewritten());

        let mut inserted = lowered.clone();
        inserted.covers = Some((0x12, 0x12));
        assert!(inserted.inserted());
        assert!(inserted.source().is_none());
        assert_eq!(inserted.id(), None);
        inserted.symbol = Some(true);
        assert_eq!(inserted.id(), Some(44));

        inserted.node = Some(Arc::new(Node::Restore(Restore::new(
            0x12,
            0x12,
            0,
            Effects::no_effect(),
        ))));
        inserted.what = Some(Semantics::new(Operation::Restore));
        assert!(!inserted.inserted());
        assert!(inserted.source().is_some());
    }

    #[test]
    fn direct_lir_model_without_gives_removed_bytes_to_the_previous_owner() {
        // Port of qbopt.model.lir:without: a removed identity copy may hand
        // its exact contiguous bytes backwards, never to a later entry.
        let prior = instruction(0x10, Some((0x10, 0x12)));
        let removed = instruction(0x12, Some((0x12, 0x14)));
        let kept = without(
            &[prior, removed],
            |one| one.at == 0x12,
            None::<fn(&Arc<Insn>) -> Arc<Insn>>,
        );
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].covers, Some((0x10, 0x14)));
    }

    #[test]
    fn direct_lir_model_without_transfers_the_first_owner_to_its_successor() {
        // Port of qbopt.model.lir:without's final first-instruction case:
        // only a block's first source span can safely move forward.
        let removed = instruction(0x10, Some((0x10, 0x12)));
        let following = instruction(0x12, Some((0x12, 0x14)));
        let kept = without(
            &[removed, following],
            |one| one.at == 0x10,
            None::<fn(&Arc<Insn>) -> Arc<Insn>>,
        );
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].covers, Some((0x10, 0x14)));
    }

    #[test]
    fn direct_lir_model_without_retains_noncontiguous_or_spread_source_owners() {
        // Port of qbopt.model.lir:without byte-ownership refusals: no valid
        // predecessor or multiple disjoint spans means the copy stays.
        let detached = instruction(0x20, Some((0x20, 0x22)));
        let next = instruction(0x30, Some((0x30, 0x32)));
        let kept = without(
            &[Arc::clone(&detached), next],
            |one| one.at == 0x20,
            None::<fn(&Arc<Insn>) -> Arc<Insn>>,
        );
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[0].covers, detached.covers);

        let prior = instruction(0x10, Some((0x10, 0x12)));
        let mut spread = (*instruction(0x12, Some((0x12, 0x14)))).clone();
        spread.spread = vec![(0x12, 0x14), (0x20, 0x22)];
        let spread = Arc::new(spread);
        let kept = without(
            &[prior, spread],
            |one| one.at == 0x12,
            None::<fn(&Arc<Insn>) -> Arc<Insn>>,
        );
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[1].covers, Some((0x12, 0x14)));
    }

    #[test]
    fn direct_lir_model_without_preserves_python_instruction_identity() {
        // Python backend passes use id(one) as their set/map key. A kept or
        // refused occurrence must therefore be the same object; ownership
        // donation is the one case that replaces the recipient dataclass.
        let kept = instruction(0x08, Some((0x08, 0x0a)));
        let recipient = instruction(0x10, Some((0x10, 0x12)));
        let donor = instruction(0x12, Some((0x12, 0x14)));
        let result = without(
            &[Arc::clone(&kept), Arc::clone(&recipient), donor],
            |one| one.at == 0x12,
            None::<fn(&Arc<Insn>) -> Arc<Insn>>,
        );
        assert!(Arc::ptr_eq(&result[0], &kept));
        assert!(!Arc::ptr_eq(&result[1], &recipient));
        assert_eq!(result[1].covers, Some((0x10, 0x14)));

        let mut refused = (*instruction(0x20, Some((0x20, 0x22)))).clone();
        refused.spread = vec![(0x20, 0x22), (0x30, 0x32)];
        let refused = Arc::new(refused);
        let result = without(
            &[Arc::clone(&kept), Arc::clone(&refused)],
            |one| one.at == 0x20,
            None::<fn(&Arc<Insn>) -> Arc<Insn>>,
        );
        assert!(Arc::ptr_eq(&result[0], &kept));
        assert!(Arc::ptr_eq(&result[1], &refused));
    }

    #[test]
    fn without_asks_drop_of_the_first_survivor_even_with_no_heir() {
        // Python evaluates `drop(first)` before `second is not None`; the
        // port skipped that call, so drop saw [0, 2] instead of [0, 2, 0].
        let first = instruction(0, Some((0, 2)));
        let bare = instruction(2, None);
        let calls = std::cell::RefCell::new(Vec::new());
        let _ = without(
            &[first, bare],
            |one| {
                calls.borrow_mut().push(one.at);
                false
            },
            None::<fn(&Arc<Insn>) -> Arc<Insn>>,
        );
        assert_eq!(*calls.borrow(), vec![0, 2, 0]);
    }

    #[test]
    fn name_of_an_unnamed_semantics_is_none() {
        // Python returns `what.name`, which is None here; the port said "".
        let one = instruction(0, None);
        assert_eq!(one.name(), None);
    }

    #[test]
    fn stage_dump_reprs_match_python() {
        // Expected strings printed by compile._lir_text's pieces in Python.
        use crate::model::ir::Loc;
        use crate::support::pyrepr::{self, Repr};
        use iced_x86::Register;

        let phi = |incoming: Vec<(i64, u32)>| super::Phi { result: 3, incoming }.repr();
        assert_eq!(phi(vec![(1, 2), (4, 5)]), "Phi(result=3, incoming=((1, 2), (4, 5)))");
        assert_eq!(phi(vec![(1, 2)]), "Phi(result=3, incoming=((1, 2),))");
        assert_eq!(phi(vec![]), "Phi(result=3, incoming=())");

        let mut one = Insn::new(
            16,
            Some((16, 18)),
            Some(Semantics {
                name: Some("call".to_owned()),
                sources: vec![Loc::Held(Held { value: 1, width: 2 })],
                target: Some(5),
                ..Semantics::new(Operation::Call)
            }),
            Vec::new(),
            Vec::new(),
        );
        one.requires = vec![
            (Held { value: 1, width: 2 }, Register::CX),
            (Held { value: 7, width: 4 }, Register::EBX),
        ];
        one.delivers = vec![(Held { value: 9, width: 1 }, Register::AL)];
        let line = |one: &Insn| {
            format!(
                "  {:4} {} req={} del={}",
                one.at,
                one.what.repr(),
                pyrepr::tuple(&one.requires),
                pyrepr::tuple(&one.delivers)
            )
        };
        assert_eq!(
            line(&one),
            "    16 Semantics(op=<Operation.CALL: 'call'>, name='call', dests=(), \
             sources=(Held(value=1, width=2),), target=5, indirect=False) \
             req=((Held(value=1, width=2), 22), (Held(value=7, width=4), 40)) \
             del=((Held(value=9, width=1), 1),)"
        );
        assert_eq!(line(&Insn::new(3, None, None, Vec::new(), Vec::new())), "     3 None req=() del=()");
        assert_eq!(pyrepr::tuple::<i64>(&[2]), "(2,)");
        assert_eq!(pyrepr::tuple::<i64>(&[2, 3]), "(2, 3)");
    }
}
