use std::sync::Arc;

use iced_x86::Register;

use super::*;
use crate::model::ir::{Held, Mem};
use crate::model::lir::{Insn, LirBlock, LirBody};

/// A load of `cell`, then a return: a body as the frame rewriters receive it.
fn loading(cell: Mem) -> LirBody {
    let semantics = |op, name: &str, dests, sources| Some(Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) });
    let load = Insn::new(0, Some((0, 1)), semantics(Operation::Move, "mov", vec![Loc::Held(Held { value: 1, width: 2 })], vec![Loc::Mem(cell)]), vec![], vec![]);
    let leave = Insn::new(1, Some((1, 2)), semantics(Operation::Return, "retf", vec![], vec![]), vec![], vec![]);
    LirBody::new("procedure", 0, vec![LirBlock::new(0, vec![Arc::new(load), Arc::new(leave)])], IndexMap::default(), IndexMap::default())
}

/// The loaded cell, after a rewrite.
fn loaded(body: &LirBody) -> Mem {
    let mut sources = body.blocks.iter().flat_map(|block| &block.insns).filter_map(|one| one.what.as_ref()).flat_map(|what| &what.sources);
    sources.find_map(|source| if let Loc::Mem(cell) = source { Some(cell.clone()) } else { None }).expect("a load")
}

/// An indexed frame cell as isel and addressforms spell it, `[bp+si-30]`:
/// through BP, with a literal displacement.
fn indexed_frame_cell() -> Mem {
    let addr = Addr { segment: Register::SS, ..Addr::new(Space::Literal, -30) };
    Mem { through: Register::BP, disp_width: 2, index: Some(Held { value: 2, width: 2 }), ..Mem::new(Some(addr), 2) }
}

/// UBOUND of a local REDIM array printed -1: its bounds' indexed cell was
/// not moved below B$ENRA's header with the rest of the frame (#80).
#[test]
fn test_an_indexed_frame_cell_moves_below_the_runtime_header() {
    let (framed, _) = _runtime_frame(&loading(indexed_frame_cell()), 46, model::RuntimeProfile::Qb45, 0).unwrap();
    let cell = loaded(&framed);
    assert_eq!((cell.through, cell.addr.unwrap().disp), (Register::BP, -30 - 10));
}

/// The module body's frame is static data, and an indexed frame cell there
/// stayed BP-relative: the runtime's BP.
#[test]
fn test_an_indexed_frame_cell_moves_into_the_static_module_frame() {
    let cell = loaded(&_static_frame(&loading(indexed_frame_cell()), 46));
    let addr = cell.addr.unwrap();
    assert_eq!((cell.through, addr.space, addr.index, addr.disp), (Register::None, Space::Segment, MAIN_FRAME_ID, 46 - 30));
}
