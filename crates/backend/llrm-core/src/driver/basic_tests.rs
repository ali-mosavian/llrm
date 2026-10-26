use std::sync::Arc;

use llrm_lir::registers::RegId;

use super::*;
use crate::model::ir::{Held, Mem, Reg};
use crate::model::lir::{Insn, LirBlock, LirBody};

/// A load of `cell`, then a return: a body as the frame rewriters receive it.
fn loading(cell: Mem) -> LirBody {
    loading_with(cell, false)
}

/// `loading`, after a load of the data segment register where `writes_ds`:
/// allocation held a selector in it.
fn loading_with(
    cell: Mem,
    writes_ds: bool,
) -> LirBody {
    let semantics = |op, name: &str, dests, sources| {
        Some(Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) })
    };
    let load = Insn::new(
        0,
        Some((0, 1)),
        semantics(Operation::Move, "mov", vec![Loc::Held(Held { value: 1, width: 2 })], vec![Loc::Mem(cell)]),
        vec![],
        vec![],
    );
    let leave = Insn::new(1, Some((1, 2)), semantics(Operation::Return, "retf", vec![], vec![]), vec![], vec![]);
    let mut insns = vec![Arc::new(load)];
    if writes_ds {
        let selector = Loc::Reg(Reg { register: RegId::DS, width: 2 });
        let set =
            semantics(Operation::Move, "mov", vec![selector], vec![Loc::Reg(Reg { register: RegId::AX, width: 2 })]);
        insns.push(Arc::new(Insn::new(2, Some((2, 3)), set, vec![], vec![])));
    }
    insns.push(Arc::new(leave));
    LirBody::new("procedure", 0, vec![LirBlock::new(0, insns).into()], IndexMap::default(), IndexMap::default())
}

/// The loaded cell, after a rewrite.
fn loaded(body: &LirBody) -> Mem {
    let mut sources = body
        .blocks
        .iter()
        .flat_map(|block| &block.insns)
        .filter_map(|one| one.what.as_ref())
        .flat_map(|what| &what.sources);
    sources.find_map(|source| if let Loc::Mem(cell) = source { Some(cell.clone()) } else { None }).expect("a load")
}

/// An indexed frame cell as isel spells it, `[bp+si-30]`: a frame cell,
/// through BP, with an index.
fn indexed_frame_cell() -> Mem {
    let addr = Addr::new(Space::Frame, -30);
    Mem { through: RegId::BP, disp_width: 2, index: Some(Held { value: 2, width: 2 }), ..Mem::new(Some(addr), 2) }
}

/// UBOUND of a local REDIM array printed -1: its bounds' indexed cell was
/// not moved below B$ENRA's header with the rest of the frame (#80).
#[test]
fn test_an_indexed_frame_cell_moves_below_the_runtime_header() {
    let (framed, _) =
        _runtime_frame(&loading(indexed_frame_cell()), 46, model::RuntimeProfile::Qb45, 0, "B$ENRA", 16).unwrap();
    let cell = loaded(&framed);
    assert_eq!((cell.through, cell.addr.unwrap().disp), (RegId::BP, -30 - 10));
}

/// The module body's frame is static data, and an indexed frame cell there
/// stayed BP-relative: the runtime's BP.
#[test]
fn test_an_indexed_frame_cell_moves_into_the_static_module_frame() {
    let cell = loaded(&_static_frame(&loading(indexed_frame_cell()), 46, None));
    let addr = cell.addr.unwrap();
    assert_eq!(
        (cell.through, addr.space, addr.index, addr.disp),
        (RegId::None, Space::Segment, MAIN_FRAME_ID, 46 - 30)
    );
}

/// The static module frame's cells read through DS, where allocation held a
/// far pointer's selector (particle.bas, #2585): spills went into an array.
#[test]
fn test_a_static_frame_cell_stays_on_the_stack_segment_where_the_stack_is_data() {
    let body = loading_with(indexed_frame_cell(), true);
    let cell = loaded(&_static_frame(&body, 46, Some(RegId::SS)));
    assert_eq!(RegId::from(cell.addr.unwrap().segment), RegId::SS);
}

/// Nothing writes DS, so it holds the data group: a frame cell needs no `ss:`
/// (pow_stack_depth carried 52 of them, a byte each).
#[test]
fn test_a_static_frame_cell_has_no_segment_prefix_where_ds_is_never_written() {
    let cell = loaded(&_static_frame(&loading(indexed_frame_cell()), 46, Some(RegId::SS)));
    assert_eq!(RegId::from(cell.addr.unwrap().segment), RegId::None);
}
