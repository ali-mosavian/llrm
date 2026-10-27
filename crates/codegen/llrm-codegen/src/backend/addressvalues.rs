//! Spells the `address` operations of an allocated body as `lea` operands.
//!
//! # How it works
//! Before allocation an address-valued operation (`Operation::Address`)
//! names its memory cell as a `Mem` source. `lea` does not read memory, so
//! once registers are chosen each such source becomes an `Address` operand
//! (`[through + index_through*scale + offset]`) and no cell is read.
//!
//! A frame cell with a base value becomes the frame register plus the cell's
//! register as index, at the cell's displacement. A cell with an address and
//! a base register keeps the address and takes that register as its base. Any
//! other cell keeps its address and registers. Instructions that are not
//! `Address`, and sources that are not `Mem`, pass through.
//!
//! ```text
//!  lea v9, Mem[frame-6, base v1, through si, scale 2]
//!    ->  lea v9, [bp + si*2 - 6]
//! ```
//!
//! # Example
//! An `Address` operation whose source is a frame cell with a base value,
//! before and after (`Address` registers print as iced-x86 `Register` numbers:
//! 26 is `BP`, 27 is `SI`):
//!
//! Before:
//!
//! ```lir
//! lea v9, Mem(addr=frame-6, through=si, offset=3, disp_width=1, base=v1, scale=2), v4
//! ```
//!
//! After:
//!
//! ```lir
//! lea v9, Address(addr=None, through=26, index=27, scale=2, offset=-6, disp_width=1), v4
//! ```
//!
//! The frame register is the base, the cell's register the index, and the frame
//! displacement replaces the cell's offset.
//!
//! From unit test `address_cells_convert_as_python_does` in `addressvalues.rs`
//! (first case, abridged from the asserted text).
//!
//! # Used by
//! `assemble.rs`, on each machined body before `masm::cleaned_returns`, and
//! the BASIC driver (`driver/basic.rs`).

use std::sync::Arc;

use llrm_lir::registers::RegId;

use crate::model::ir::{AddressRef, Loc, Operation};
use crate::model::lir::{Insn, LirBody};
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
                sources.push(Loc::Address(AddressRef {
                    addr: None,
                    through: crate::model::ir::FRAME,
                    index_through: source.through,
                    scale: source.scale,
                    offset: addr.expect("checked").disp,
                    disp_width: source.disp_width,
                }));
                continue;
            }
            if addr.is_some() && source.base.is_some() && source.through != RegId::None {
                addr = addr.map(|addr| Addr { base: source.through.iced(), ..addr });
            }
            sources.push(Loc::Address(AddressRef {
                addr,
                through: source.through,
                index_through: source.index_through,
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
    converted.blocks =
        body.blocks.iter().map(|block| block.with_insns(block.insns.iter().map(instruction).collect())).collect();
    converted
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use llrm_lir::registers::RegId;

    use super::converted;
    use crate::model::ir::{Addr, Held, Loc, Mem, Operation, Semantics, Space};
    use crate::model::lir::{Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;
    use crate::support::pyrepr::Repr;

    #[test]
    fn address_cells_convert_as_python_does() {
        // No Python test covers this module; expected text is Python's repr
        // of the same input.
        let semantics = |op, dests, sources| Semantics {
            name: Some(if op == Operation::Address { "lea" } else { "mov" }.into()),
            dests,
            sources,
            ..Semantics::new(op)
        };
        let held = |value| Loc::Held(Held { value, width: 2 });
        let mem = |addr, through, offset, disp_width| Mem { through, offset, disp_width, ..Mem::new(Some(addr), 2) };
        let frame = Mem {
            base: Some(Held { value: 1, width: 2 }),
            scale: 2,
            ..mem(Addr::new(Space::Frame, -6), RegId::SI, 3, 1)
        };
        let seg = Mem {
            base: Some(Held { value: 2, width: 2 }),
            index_through: RegId::DI,
            ..mem(Addr::new(Space::Segment, 8), RegId::BX, 5, 2)
        };
        let plain = mem(Addr::new(Space::Segment, 8), RegId::BX, 5, 2);
        let insn = |at, what| Arc::new(Insn::new(at, Some((at, at)), Some(what), Vec::new(), Vec::new()));
        let body = LirBody::new(
            "f",
            0,
            vec![
                LirBlock::new(
                    0,
                    vec![
                        insn(0, semantics(Operation::Address, vec![held(9)], vec![Loc::Mem(frame), held(4)])),
                        insn(
                            1,
                            semantics(Operation::Address, vec![held(9)], vec![Loc::Mem(seg.clone()), Loc::Mem(plain)]),
                        ),
                        insn(2, semantics(Operation::Move, vec![held(9)], vec![Loc::Mem(seg)])),
                    ],
                )
                .into(),
            ],
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
