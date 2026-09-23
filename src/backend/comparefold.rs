//! Port of `qbopt/backend/comparefold.py`: select one-use memory comparisons
//! before register allocation.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::support::hash::{IndexMap, IndexSet};

use crate::model::ir::{self, Held, Loc, Operation};
use crate::model::lir::{self, Insn};

/// Fold a private load followed by its sole comparison into a memory comparison.
///
/// A widening load may fold only for an equality test against zero:
/// narrowing that comparison preserves ZF, while a signed condition could
/// observe the narrow cell's top bit through SF.
///
/// `users` is Python's `Counter`: a missing value counts zero.
pub fn selected(insns: &[Arc<Insn>], users: &IndexMap<u32, i64>, exposed: &BTreeSet<u32>) -> Vec<Arc<Insn>> {
    let mut out = insns.to_vec();
    for (load_at, load) in insns.iter().enumerate() {
        let mut extension = false;
        let (value, width, cell) = match &load.what {
            Some(what)
                if what.op == Operation::Move
                    && what.name.as_deref() == Some("mov")
                    && matches!(what.dests.as_slice(), [Loc::Held(_)])
                    && matches!(what.sources.as_slice(), [Loc::Mem(_)]) =>
            {
                let (Loc::Held(Held { value, width }), Loc::Mem(cell)) = (&what.dests[0], &what.sources[0]) else {
                    unreachable!()
                };
                (*value, *width, cell.clone())
            }
            Some(what)
                if what.op == Operation::Extend
                    && matches!(what.name.as_deref(), Some("movsx" | "movzx"))
                    && matches!(what.dests.as_slice(), [Loc::Held(_)])
                    && matches!(what.sources.as_slice(), [Loc::Mem(_)]) =>
            {
                let (Loc::Held(Held { value, width }), Loc::Mem(cell)) = (&what.dests[0], &what.sources[0]) else {
                    unreachable!()
                };
                if cell.width >= *width {
                    continue;
                }
                extension = true;
                (*value, *width, cell.clone())
            }
            _ => continue,
        };
        if (cell.width != width && !extension)
            || load.defines != [value]
            || users.get(&value).copied().unwrap_or(0) != 1
            || exposed.contains(&value)
            || !_plain(load)
        {
            continue;
        }

        let mut compare_at = load_at + 1;
        while compare_at < insns.len() && _anchor(&insns[compare_at]) {
            compare_at += 1;
        }
        if compare_at == insns.len() {
            continue;
        }
        let compare = &insns[compare_at];
        let sources = match &compare.what {
            Some(what)
                if what.op == Operation::Compare && what.name.as_deref() == Some("cmp") && what.dests.is_empty() =>
            {
                what.sources.clone()
            }
            _ => continue,
        };
        if sources.len() != 2 || !_plain(compare) || !compare.defines.is_empty() {
            continue;
        }
        let positions: Vec<usize> = sources
            .iter()
            .enumerate()
            .filter(|(_, source)| **source == Loc::Held(Held { value, width }))
            .map(|(index, _)| index)
            .collect();
        if positions.len() != 1 {
            continue;
        }
        let position = positions[0];
        let mut other = sources[1 - position].clone();
        // `getattr(other, "width", None)`.
        let other_width = match &other {
            Loc::Reg(one) => Some(one.width),
            Loc::Mem(one) => Some(one.width),
            Loc::Imm(one) => Some(one.width),
            Loc::Held(one) => Some(one.width),
            Loc::Address(_) | Loc::St(_) => None,
        };
        if matches!(other, Loc::Mem(_)) || other_width != Some(width) {
            continue;
        }
        if extension {
            if !matches!(&other, Loc::Imm(imm) if imm.value == 0 && imm.address.is_none()) {
                continue;
            }
            let mut branch_at = compare_at + 1;
            while branch_at < insns.len() && _anchor(&insns[branch_at]) {
                branch_at += 1;
            }
            if branch_at == insns.len() {
                continue;
            }
            let branch = &insns[branch_at];
            if branch.what.as_ref().is_none_or(|what| {
                what.op != Operation::Branch || !matches!(what.name.as_deref(), Some("je" | "jne"))
            }) {
                continue;
            }
            other = Loc::Imm(ir::Imm { value: 0, width: cell.width, address: None });
        }
        // A displacement and immediate can each own a relocation.  LIR's
        // ownership flag names one source operation, so retain the unfused
        // pair rather than silently dropping either fixup.
        if load.symbol == Some(true) && compare.symbol == Some(true) {
            continue;
        }

        let mut folded_sources = sources.clone();
        folded_sources[position] = Loc::Mem(cell.clone());
        folded_sources[1 - position] = other;
        let address_uses: Vec<u32> = ir::values(&Loc::Mem(cell.clone())).iter().map(|held| held.value).collect();
        let compare_uses: Vec<u32> = compare.uses.iter().copied().filter(|one| *one != value).collect();
        let mut folded = (**compare).clone();
        let mut what = compare.what.clone().expect("matched above");
        what.sources = folded_sources;
        folded.what = Some(what);
        folded.uses = address_uses
            .into_iter()
            .chain(compare_uses)
            .collect::<IndexSet<u32>>()
            .into_iter()
            .collect();
        folded.op = if load.symbol == Some(true) { load.op.clone() } else { compare.op.clone() };
        folded.symbol = if load.symbol == Some(true) { Some(true) } else { compare.symbol };
        out[compare_at] = Arc::new(folded);
        // Keep source ownership and anchors while deleting the virtual range
        // before allocation.  This is the same ownership transaction used by
        // pre-allocation RMW selection.
        let mut anchored = (*lir::anchor(Arc::clone(load))).clone();
        anchored.defines = Vec::new();
        anchored.uses = Vec::new();
        anchored.widths = Vec::new();
        out[load_at] = Arc::new(anchored);
    }
    out
}

fn _plain(one: &Insn) -> bool {
    !(!one.clobbers.is_empty()
        || !one.clobbers_high.is_empty()
        || !one.requires.is_empty()
        || !one.delivers.is_empty()
        || !one.spread.is_empty()
        || one.group.is_some()
        || one.frame_adjust
        || one.spill_reload
        || one.spill_store
        || one.rematerialized)
}

fn _anchor(one: &Insn) -> bool {
    one.what.as_ref().is_some_and(|what| what.op == Operation::Nothing)
        && one.defines.is_empty()
        && one.uses.is_empty()
        && _plain(one)
}

#[cfg(test)]
mod tests {
    //! Port of the comparefold test in `tests/test_memory_folding.py`.

    use std::collections::BTreeSet;
    use std::sync::Arc;

    use iced_x86::Register;
    use crate::support::hash::IndexMap;

    use super::selected;
    use crate::model::ir::{Addr, Held, Imm, Loc, Mem, Operation, Semantics, Space};
    use crate::model::lir::Insn;

    #[test]
    fn test_one_use_memory_comparison_is_selected_before_allocation() {
        // Retaining lru_use's test temporary made its surviving far pointer spill.
        let (base, selector, loaded, other) = (1, 2, 3, 4);
        let cell = Mem {
            through: Register::None,
            offset: 0,
            disp_width: 2,
            base: Some(Held { value: base, width: 2 }),
            selector: Some(Held { value: selector, width: 2 }),
            ..Mem::new(Some(Addr { segment: Register::ES, ..Addr::new(Space::Far, 0) }), 2)
        };
        let semantics = |op, name: &str, dests, sources| Semantics {
            name: Some(name.to_owned()),
            dests,
            sources,
            ..Semantics::new(op)
        };
        let load = Arc::new(Insn::new(
            1,
            None,
            Some(semantics(
                Operation::Move,
                "mov",
                vec![Loc::Held(Held { value: loaded, width: 2 })],
                vec![Loc::Mem(cell.clone())],
            )),
            vec![loaded],
            vec![base, selector],
        ));
        let anchor = Arc::new(Insn::new(
            2,
            None,
            Some(semantics(Operation::Nothing, "", Vec::new(), Vec::new())),
            Vec::new(),
            Vec::new(),
        ));
        let compare = Arc::new(Insn::new(
            3,
            None,
            Some(semantics(
                Operation::Compare,
                "cmp",
                Vec::new(),
                vec![Loc::Held(Held { value: loaded, width: 2 }), Loc::Held(Held { value: other, width: 2 })],
            )),
            Vec::new(),
            vec![loaded, other],
        ));
        let users: IndexMap<u32, i64> = [(base, 1), (selector, 1), (loaded, 1), (other, 1)].into_iter().collect();

        let result = selected(&[load, anchor, compare], &users, &BTreeSet::new());

        assert_eq!(result[0].what.as_ref().unwrap().op, Operation::Nothing);
        assert!(result[0].defines.is_empty() && result[0].uses.is_empty());
        assert_eq!(
            result[2].what.as_ref().unwrap().sources,
            vec![Loc::Mem(cell), Loc::Held(Held { value: other, width: 2 })]
        );
        assert_eq!(result[2].uses, vec![base, selector, other]);
    }

    fn widened(loaded: u32, cell: &Mem, uses: Vec<u32>) -> [Arc<Insn>; 2] {
        let load = Insn::new(
            1,
            None,
            Some(Semantics {
                name: Some("movzx".to_owned()),
                dests: vec![Loc::Held(Held { value: loaded, width: 2 })],
                sources: vec![Loc::Mem(cell.clone())],
                ..Semantics::new(Operation::Extend)
            }),
            vec![loaded],
            uses,
        );
        let compare = Insn::new(
            2,
            None,
            Some(Semantics {
                name: Some("cmp".to_owned()),
                sources: vec![
                    Loc::Held(Held { value: loaded, width: 2 }),
                    Loc::Imm(Imm { value: 0, width: 2, address: None }),
                ],
                ..Semantics::new(Operation::Compare)
            }),
            Vec::new(),
            vec![loaded],
        );
        [Arc::new(load), Arc::new(compare)]
    }

    fn branch(name: &str) -> Arc<Insn> {
        Arc::new(Insn::new(
            3,
            None,
            Some(Semantics { name: Some(name.to_owned()), target: Some(9), ..Semantics::new(Operation::Branch) }),
            Vec::new(),
            Vec::new(),
        ))
    }

    #[test]
    fn test_one_use_zero_extended_byte_test_is_selected_before_allocation() {
        // C sieve ran 13.4% behind BCC after loading every flag byte into AX.
        let (index, loaded) = (1, 2);
        let cell = Mem {
            through: Register::BP,
            offset: -1028,
            disp_width: 2,
            index: Some(Held { value: index, width: 2 }),
            ..Mem::new(Some(Addr::new(Space::Frame, -1028)), 1)
        };
        let [load, compare] = widened(loaded, &cell, vec![index]);
        let users: IndexMap<u32, i64> = [(index, 1), (loaded, 1)].into_iter().collect();

        let result = selected(&[load, compare, branch("je")], &users, &BTreeSet::new());

        assert_eq!(result[0].what.as_ref().unwrap().op, Operation::Nothing);
        assert_eq!(
            result[1].what.as_ref().unwrap().sources,
            vec![Loc::Mem(cell), Loc::Imm(Imm { value: 0, width: 1, address: None })]
        );
        assert_eq!(result[1].uses, vec![index]);
    }

    #[test]
    fn test_zero_extended_byte_test_keeps_the_load_when_sign_is_observed() {
        // A narrow memory compare exposes bit 7 as SF; a zero-extended word test does not.
        let loaded = 1;
        let cell = Mem {
            through: Register::BP,
            offset: -4,
            disp_width: 1,
            ..Mem::new(Some(Addr::new(Space::Frame, -4)), 1)
        };
        let [load, compare] = widened(loaded, &cell, Vec::new());
        let insns = vec![load, compare, branch("jl")];
        let users: IndexMap<u32, i64> = [(loaded, 1)].into_iter().collect();

        assert_eq!(selected(&insns, &users, &BTreeSet::new()), insns);
    }
}
