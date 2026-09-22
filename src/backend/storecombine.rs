//! Port of `qbopt/backend/storecombine.py`: pack neighboring literal word
//! stores after allocation.

use std::sync::Arc;

use iced_x86::Register;

use crate::model::ir::{Imm, Loc, Mem, Operation, Semantics, Space};
use crate::model::lir::{Insn, LirBody};

fn _plain(one: &Insn) -> bool {
    !(!one.clobbers.is_empty()
        || !one.requires.is_empty()
        || !one.delivers.is_empty()
        || !one.defines.is_empty()
        || !one.uses.is_empty()
        || !one.spread.is_empty()
        || one.group.is_some()
        || one.symbol == Some(true))
}

fn _literal(one: &Insn) -> Option<(Mem, i64)> {
    if !_plain(one) {
        return None;
    }
    let what = one.what.as_ref()?;
    if what.op != Operation::Move || what.name.as_deref() != Some("mov") {
        return None;
    }
    let ([Loc::Mem(cell)], [Loc::Imm(Imm { value: number, width: 2, address: None })]) =
        (what.dests.as_slice(), what.sources.as_slice())
    else {
        return None;
    };
    let address = cell.addr.as_ref()?;
    if cell.width == 2
        && cell.base.is_none()
        && cell.through == Register::None
        && cell.offset == 0
        && address.space == Space::Segment
        && address.base == Register::None
        && address.segment == Register::None
        && (0..=0xfffe).contains(&address.disp)
    {
        return Some((cell.clone(), *number));
    }
    None
}

#[must_use]
pub fn combined(body: &LirBody) -> LirBody {
    let mut out = body.clone();
    let nothing = Semantics {
        name: Some(String::new()),
        ..Semantics::new(Operation::Nothing)
    };
    for block in &mut out.blocks {
        let mut insns = block.insns.clone();
        let mut pending: Option<(usize, (Mem, i64))> = None;
        for index in 0..insns.len() {
            let one = Arc::clone(&insns[index]);
            if one.what.as_ref() == Some(&nothing) && _plain(&one) {
                continue;
            }
            let current = _literal(&one);
            if let (Some((high_cell, high)), Some((previous, (low_cell, low)))) = (&current, &pending) {
                let low_addr = low_cell.addr.as_ref().expect("a literal store has an address");
                if high_cell.addr == Some(low_addr.plus(2)) && low_addr.disp <= 0xfffc {
                    let first = &insns[*previous];
                    let mut what = first.what.clone().expect("a literal store has semantics");
                    what.dests = vec![Loc::Mem(Mem { width: 4, ..low_cell.clone() })];
                    what.sources = vec![Loc::Imm(Imm {
                        value: (low & 0xffff) | ((high & 0xffff) << 16),
                        width: 4,
                        address: None,
                    })];
                    let mut joined = (**first).clone();
                    joined.what = Some(what);
                    joined.symbol = Some(false);
                    insns[*previous] = Arc::new(joined);
                    let mut emptied = (*one).clone();
                    emptied.what = Some(nothing.clone());
                    emptied.symbol = Some(false);
                    insns[index] = Arc::new(emptied);
                    pending = None;
                    continue;
                }
            }
            pending = current.map(|current| (index, current));
        }
        block.insns = insns;
    }
    out
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use iced_x86::Register;
    use crate::support::hash::IndexMap;

    use super::combined;
    use crate::model::ir::{Addr, Held, Imm, Loc, Mem, Operation, Reg, Semantics, Space};
    use crate::model::lir::{Insn, LirBlock, LirBody};

    fn what(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> Option<Semantics> {
        Some(Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) })
    }

    fn imm(value: i64, width: u32, address: Option<Addr>) -> Loc {
        Loc::Imm(Imm { value, width, address })
    }

    fn with_dest(one: &Insn, dest: Mem) -> Insn {
        let mut out = one.clone();
        out.what.as_mut().unwrap().dests = vec![Loc::Mem(dest)];
        out
    }

    #[test]
    fn test_only_adjacent_unobserved_static_literals_are_packed() {
        // Packing x=-1,t=2 must not cross a read, call or uncertain address.
        let guards = [
            None,
            Some("gap"),
            Some("segment"),
            Some("indexed"),
            Some("external"),
            Some("symbol"),
            Some("call"),
            Some("read"),
            Some("requirements"),
            Some("block"),
        ];
        for guard in guards {
            let cell = Mem { disp_width: 2, ..Mem::new(Some(Addr { index: 5, ..Addr::new(Space::Segment, 14) }), 2) };
            let cell_addr = cell.addr.clone().unwrap();
            let mut low = Insn::new(0, Some((0, 6)), what(Operation::Move, "mov", vec![Loc::Mem(cell.clone())], vec![imm(-1, 2, None)]), vec![], vec![]);
            let high_cell = Mem { addr: Some(cell_addr.plus(2)), ..cell.clone() };
            let mut high = low.clone();
            high.at = 8;
            high.covers = Some((8, 14));
            high.what = what(Operation::Move, "mov", vec![Loc::Mem(high_cell.clone())], vec![imm(2, 2, None)]);
            let mut marker = Insn::new(6, Some((6, 8)), what(Operation::Nothing, "", vec![], vec![]), vec![], vec![]);
            match guard {
                Some("gap") => high = with_dest(&high, Mem { addr: Some(cell_addr.plus(4)), ..high_cell.clone() }),
                Some("segment") => {
                    high = with_dest(&high, Mem { addr: Some(Addr { index: 6, ..high_cell.addr.clone().unwrap() }), ..high_cell.clone() });
                }
                Some("indexed") => low = with_dest(&low, Mem { through: Register::BX, ..cell.clone() }),
                Some("external") => {
                    low = with_dest(&low, Mem { addr: Some(Addr { space: Space::External, ..cell_addr.clone() }), ..cell.clone() });
                }
                Some("symbol") => high.what.as_mut().unwrap().sources = vec![imm(2, 2, Some(cell_addr.clone()))],
                Some("call") => marker.what = what(Operation::Call, "call", vec![], vec![]),
                Some("read") => {
                    marker.what = what(
                        Operation::Move,
                        "mov",
                        vec![Loc::Reg(Reg { register: Register::AX, width: 2 })],
                        vec![Loc::Mem(cell.clone())],
                    );
                }
                Some("requirements") => high.requires = vec![(Held { value: 1, width: 2 }, Register::AX)],
                _ => {}
            }
            let (low, marker, high) = (Arc::new(low), Arc::new(marker), Arc::new(high));
            let blocks = if guard == Some("block") {
                vec![
                    LirBlock { succ: vec![8], ..LirBlock::new(0, vec![low, marker]) },
                    LirBlock::new(8, vec![high]),
                ]
            } else {
                vec![LirBlock::new(0, vec![low, marker, high])]
            };
            let body = LirBody::new("stores", 0, blocks, IndexMap::default(), IndexMap::default());
            let result = combined(&body);
            if guard.is_some() {
                assert_eq!(result, body, "{guard:?}");
            } else {
                let insns = result.insns();
                let first = insns[0].what.as_ref().unwrap();
                assert_eq!(first.dests, vec![Loc::Mem(Mem { width: 4, ..cell.clone() })]);
                assert_eq!(first.sources, vec![imm(0x2ffff, 4, None)]);
                assert_eq!(insns[insns.len() - 1].what.as_ref().unwrap().op, Operation::Nothing);
                let covers = |body: &LirBody| body.insns().iter().map(|one| one.covers).collect::<Vec<_>>();
                assert_eq!(covers(&result), covers(&body));
            }
        }
    }
}
