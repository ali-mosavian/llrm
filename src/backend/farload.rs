//! Port of `qbopt/backend/farload.py`: select complete far-pointer loads
//! before allocation loses their address.

use std::collections::BTreeSet;
use std::sync::Arc;

use indexmap::{IndexMap, IndexSet};

use crate::model::ir::{self, Held, Loc, Mem, Operation, Semantics};
use crate::model::lir::{self, Insn};

/// Select private `mov offset,[p]; mov selector,[p+2]` pairs as `les`.
pub fn selected(insns: &[Arc<Insn>]) -> Vec<Arc<Insn>> {
    let mut made: IndexMap<usize, Arc<Insn>> = IndexMap::new();
    let mut erased: BTreeSet<usize> = BTreeSet::new();
    for (at, first) in insns[..insns.len().saturating_sub(1)].iter().enumerate() {
        if erased.contains(&at) {
            continue;
        }
        let second = &insns[at + 1];
        let Some(joined) = _pair(first, second) else {
            continue;
        };
        made.insert(at, joined);
        erased.insert(at + 1);
    }
    insns
        .iter()
        .enumerate()
        .map(|(at, one)| {
            if let Some(joined) = made.get(&at) {
                Arc::clone(joined)
            } else if erased.contains(&at) {
                let mut anchored = (*lir::anchor(Arc::clone(one))).clone();
                anchored.defines = Vec::new();
                anchored.uses = Vec::new();
                anchored.widths = Vec::new();
                Arc::new(anchored)
            } else {
                Arc::clone(one)
            }
        })
        .collect()
}

fn _pair(first: &Insn, second: &Insn) -> Option<Arc<Insn>> {
    if !_plain(first)
        || !_plain(second)
        || first.covers.is_some_and(|covers| covers.0 != covers.1)
        || second.covers.is_some_and(|covers| covers.0 != covers.1)
    {
        return None;
    }
    let mut words: Vec<(Held, Mem)> = Vec::new();
    for one in [first, second] {
        match &one.what {
            Some(what)
                if what.op == Operation::Move
                    && what.name.as_deref() == Some("mov")
                    && matches!(what.dests.as_slice(), [Loc::Held(_)])
                    && matches!(what.sources.as_slice(), [Loc::Mem(_)]) =>
            {
                let (Loc::Held(dest), Loc::Mem(cell)) = (&what.dests[0], &what.sources[0]) else {
                    unreachable!()
                };
                // Python's chained `dest.width != cell.width != 2`.
                if dest.width != cell.width && cell.width != 2 {
                    return None;
                }
                words.push((*dest, cell.clone()));
            }
            _ => return None,
        }
    }
    let (first_dest, first_cell) = words[0].clone();
    let (second_dest, second_cell) = words[1].clone();
    if !_next_word(&first_cell, &second_cell) || !_far_pointer_words(first, second) {
        return None;
    }
    // A fixed address survives allocation unchanged; only a virtual
    // base/selector/index needs the early complete-load selection.
    if ir::values(&Loc::Mem(first_cell.clone())).is_empty() {
        return None;
    }
    let defined: BTreeSet<u32> = [first_dest.value, second_dest.value].into_iter().collect();
    if [&first_cell, &second_cell]
        .into_iter()
        .any(|cell| ir::values(&Loc::Mem(cell.clone())).iter().any(|value| defined.contains(&value.value)))
    {
        return None;
    }
    // C's far pointer is low offset then high selector; the allocator's
    // selector order prefers ES, making LES the one-instruction load.
    let what = Semantics {
        name: Some("les".to_owned()),
        dests: vec![Loc::Held(first_dest), Loc::Held(second_dest)],
        sources: vec![Loc::Mem(Mem { width: 4, ..first_cell })],
        ..Semantics::new(Operation::Move)
    };
    let mut joined = first.clone();
    joined.what = Some(what);
    joined.defines = first
        .defines
        .iter()
        .chain(&second.defines)
        .copied()
        .collect::<IndexSet<u32>>()
        .into_iter()
        .collect();
    joined.uses = first
        .uses
        .iter()
        .chain(&second.uses)
        .copied()
        .collect::<IndexSet<u32>>()
        .into_iter()
        .collect();
    Some(Arc::new(joined))
}

fn _plain(one: &Insn) -> bool {
    !(!one.clobbers.is_empty()
        || !one.clobbers_high.is_empty()
        || !one.requires.is_empty()
        || !one.delivers.is_empty()
        || !one.spread.is_empty()
        || one.group.is_some()
        || one.symbol == Some(true)
        || one.frame_adjust
        || one.spill_reload
        || one.spill_store
        || one.rematerialized)
}

/// Whether `high` is the word immediately after `low` by one address.
fn _next_word(low: &Mem, high: &Mem) -> bool {
    let same = |cell: &Mem| Mem {
        addr: cell.addr.map(|addr| ir::Addr { disp: 0, ..addr }),
        offset: 0,
        ..cell.clone()
    };
    if same(low) != same(high) {
        return false;
    }
    let (Some(low_addr), Some(high_addr)) = (low.addr, high.addr) else {
        return low.addr.is_none() && high.addr.is_none() && high.offset == low.offset + 2;
    };
    let moved = high_addr.disp - low_addr.disp;
    moved == 2 && [0, 2].contains(&(high.offset - low.offset)) || moved == 0 && high.offset == low.offset + 2
}

/// Whether the two loads are adjacent halves of one typed far pointer.
fn _far_pointer_words(first: &Insn, second: &Insn) -> bool {
    for one in [first, second] {
        let loaded: &[crate::model::mir::MemRef] = match &one.op {
            None => &[],
            Some(op) => &op.loads,
        };
        if loaded.len() != 1 || loaded[0].width != 2 || loaded[0].volatile || loaded[0].typed.is_none() {
            return false;
        }
        if loaded[0].typed.as_ref().expect("checked").0 != "pointer4" {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    //! Port of `tests/test_farload.py`.

    use std::sync::Arc;

    use super::selected;
    use crate::model::ir::{self, Addr, Loc, Mem, Operation, Semantics, Space};
    use crate::model::lir::Insn;
    use crate::model::mir::{self, Arg, Cell, Kind, MemRef, Op, OpCode, Value};

    fn typed_ref(space: Space, disp: i64) -> MemRef {
        MemRef { typed: Some(("pointer4".to_owned(), false)), ..MemRef::new(Some(Addr::new(space, disp)), 2) }
    }

    fn load(at: i64, value: Value, r#ref: MemRef, cell: Mem, uses: Vec<u32>) -> Arc<Insn> {
        let destination = ir::Held { value: value.id, width: 2 };
        let mut op = Op::new(at, Some(OpCode::Operation(Operation::Move)), "mov", vec![value], Vec::new());
        op.loads = vec![r#ref.clone()];
        op.kind = Kind::Load;
        op.args = vec![Arg::Cell(Cell { r#ref })];
        op.results = vec![Arg::Held(mir::Held { value, width: 2 })];
        let mut one = Insn::new(
            at,
            Some((at, at)),
            Some(Semantics {
                name: Some("mov".to_owned()),
                dests: vec![Loc::Held(destination)],
                sources: vec![Loc::Mem(cell)],
                ..Semantics::new(Operation::Move)
            }),
            vec![value.id],
            uses,
        );
        one.op = Some(Arc::new(op));
        Arc::new(one)
    }

    #[test]
    fn test_typed_far_pointer_words_without_object_provenance_are_joined() {
        // qcport's dynamic far-struct fields emitted two loads per far pointer.
        let (first_value, second_value) = (Value::new(1, 1), Value::new(2, 1));
        let first_ref = typed_ref(Space::Far, 4);
        let second_ref = typed_ref(Space::Far, 6);
        let cell = |r#ref: &MemRef| Mem {
            base: Some(ir::Held { value: 10, width: 2 }),
            selector: Some(ir::Held { value: 11, width: 2 }),
            ..Mem::new(r#ref.addr, 2)
        };
        let values = |cell: &Mem| ir::values(&Loc::Mem(cell.clone())).iter().map(|one| one.value).collect();
        let (first_cell, second_cell) = (cell(&first_ref), cell(&second_ref));
        let original = vec![
            load(1, first_value, first_ref, first_cell.clone(), values(&first_cell)),
            load(2, second_value, second_ref, second_cell.clone(), values(&second_cell)),
        ];

        let selected = selected(&original);

        assert_eq!(selected[0].what.as_ref().unwrap().name.as_deref(), Some("les"));
        let Loc::Mem(source) = &selected[0].what.as_ref().unwrap().sources[0] else { panic!() };
        assert_eq!(source.width, 4);
        assert_eq!(selected[0].defines, vec![1, 2]);
        assert_eq!(selected[1].what.as_ref().unwrap().op, Operation::Nothing);
    }

    #[test]
    fn test_fixed_far_pointer_load_defers_fusion_until_after_allocation() {
        // indexed.lru_use's fixed parameter pair became an eager LES.
        let values = [Value::new(1, 1), Value::new(2, 1)];
        let load = |at: i64, value: Value, displacement: i64| {
            let r#ref = typed_ref(Space::Frame, displacement);
            let cell = Mem::new(r#ref.addr, 2);
            load(at, value, r#ref, cell, Vec::new())
        };

        let original = vec![load(1, values[0], 4), load(2, values[1], 6)];

        assert_eq!(selected(&original), original);
    }
}
