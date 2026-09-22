//! Port of `qbopt/backend/addressvalues.py`: spell allocated address-valued
//! memory operands as machine addresses.

use std::sync::Arc;

use iced_x86::Register;

use crate::model::ir::{Address, Loc, Operation};
use crate::model::lir::{Insn, LirBlock, LirBody};
use crate::objectfile::module::{Addr, Space};

/// Turn allocated ADDRESS cells into LEA's non-memory operand spelling.
pub fn converted(body: &LirBody) -> LirBody {
    let instruction = |one: &Arc<Insn>| -> Arc<Insn> {
        let what = match &one.what {
            Some(what) if what.op == Operation::Address => what,
            _ => return Arc::clone(one),
        };
        let mut sources = Vec::new();
        for source in &what.sources {
            let Loc::Mem(source) = source else {
                sources.push(source.clone());
                continue;
            };
            let mut addr = source.addr;
            if addr.is_some_and(|addr| addr.space == Space::Frame) && source.base.is_some() {
                sources.push(Loc::Address(Address {
                    addr: None,
                    through: Register::BP,
                    index: source.through,
                    scale: source.scale,
                    offset: addr.expect("checked").disp,
                    disp_width: source.disp_width,
                }));
                continue;
            }
            if addr.is_some() && source.base.is_some() && source.through != Register::None {
                addr = addr.map(|addr| Addr { base: source.through, ..addr });
            }
            sources.push(Loc::Address(Address {
                addr,
                through: source.through,
                index: source.index_through,
                scale: source.scale,
                offset: source.offset,
                disp_width: source.disp_width,
            }));
        }
        let mut what = what.clone();
        what.sources = sources;
        let mut made = (**one).clone();
        made.what = Some(what);
        Arc::new(made)
    };

    let mut converted = body.clone();
    converted.blocks = body
        .blocks
        .iter()
        .map(|block| LirBlock {
            insns: block.insns.iter().map(instruction).collect(),
            ..block.clone()
        })
        .collect();
    converted
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::support::hash::IndexMap;
    use iced_x86::Register;

    use super::converted;
    use crate::model::ir::{Addr, Held, Loc, Mem, Operation, Semantics, Space};
    use crate::model::lir::{Insn, LirBlock, LirBody};
    use crate::support::pyrepr::Repr;

    #[test]
    fn address_cells_convert_as_python_does() {
        // No Python test covers this module; expected text is Python's repr
        // of the same input.
        let semantics = |op, dests, sources| Semantics {
            name: Some(if op == Operation::Address { "lea" } else { "mov" }.to_owned()),
            dests,
            sources,
            ..Semantics::new(op)
        };
        let held = |value| Loc::Held(Held { value, width: 2 });
        let mem = |addr, through, offset, disp_width| Mem {
            through,
            offset,
            disp_width,
            ..Mem::new(Some(addr), 2)
        };
        let frame = Mem {
            base: Some(Held { value: 1, width: 2 }),
            scale: 2,
            ..mem(Addr::new(Space::Frame, -6), Register::SI, 3, 1)
        };
        let seg = Mem {
            base: Some(Held { value: 2, width: 2 }),
            index_through: Register::DI,
            ..mem(Addr::new(Space::Segment, 8), Register::BX, 5, 2)
        };
        let plain = mem(Addr::new(Space::Segment, 8), Register::BX, 5, 2);
        let insn = |at, what| Arc::new(Insn::new(at, Some((at, at)), Some(what), Vec::new(), Vec::new()));
        let body = LirBody::new(
            "f",
            0,
            vec![LirBlock::new(
                0,
                vec![
                    insn(0, semantics(Operation::Address, vec![held(9)], vec![Loc::Mem(frame), held(4)])),
                    insn(1, semantics(Operation::Address, vec![held(9)], vec![Loc::Mem(seg.clone()), Loc::Mem(plain)])),
                    insn(2, semantics(Operation::Move, vec![held(9)], vec![Loc::Mem(seg)])),
                ],
            )],
            IndexMap::default(),
            IndexMap::default(),
        );

        let printed: Vec<String> = converted(&body).blocks[0].insns.iter().map(|one| one.what.repr()).collect();

        assert_eq!(
            printed,
            [
                "Semantics(op=<Operation.ADDRESS: 'addr'>, name='lea', dests=(Held(value=9, width=2),), \
                 sources=(Address(addr=None, through=26, index=27, scale=2, offset=-6, disp_width=1), \
                 Held(value=4, width=2)), target=None, indirect=False)",
                "Semantics(op=<Operation.ADDRESS: 'addr'>, name='lea', dests=(Held(value=9, width=2),), \
                 sources=(Address(addr=[seg:0+bx+0x8], through=24, index=28, scale=1, offset=5, disp_width=2), \
                 Address(addr=[seg:0+0x8], through=24, index=0, scale=1, offset=5, disp_width=2)), \
                 target=None, indirect=False)",
                "Semantics(op=<Operation.MOVE: 'move'>, name='mov', dests=(Held(value=9, width=2),), \
                 sources=(Mem(addr=[seg:0+0x8], width=2, through=24, offset=5, disp_width=2, \
                 base=Held(value=2, width=2), stack_argument=False, selector=None, index=None, scale=1, \
                 index_through=28),), target=None, indirect=False)",
            ]
        );
    }
}
