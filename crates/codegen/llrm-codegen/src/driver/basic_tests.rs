use std::sync::Arc;

use llrm_lir::registers::RegId;

use super::*;
use crate::model::ir::{Held, Mem};
use crate::model::lir::{Insn, LirBlock, LirBody};

/// A load of `cell`, then a return: a body as the frame rewriters receive it.
fn loading(cell: Mem) -> LirBody {
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
    LirBody::new(
        "procedure",
        0,
        vec![LirBlock::new(0, vec![Arc::new(load), Arc::new(leave)]).into()],
        IndexMap::default(),
        IndexMap::default(),
    )
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

/// The body of `loading` with a selector put in the data segment register
/// first, as allocation does for a far pointer.
fn holding_a_selector(cell: Mem) -> LirBody {
    let mut body = loading(cell);
    let put = Insn::new(
        0,
        Some((0, 1)),
        Some(Semantics {
            name: Some("mov".into()),
            dests: vec![Loc::Reg(ir::Reg { register: RegId::DS, width: 2 })],
            sources: vec![Loc::Held(Held { value: 3, width: 2 })],
            ..Semantics::new(Operation::Move)
        }),
        vec![],
        vec![],
    );
    let mut insns = vec![Arc::new(put)];
    insns.extend(body.blocks[0].insns.iter().cloned());
    body.blocks = vec![body.blocks[0].with_insns(insns)].into();
    body
}

/// The static module frame's cells read through DS, where allocation held a
/// far pointer's selector (particle.bas, #2585): spills went into an array.
#[test]
fn test_a_static_frame_cell_stays_on_the_stack_segment_where_the_stack_is_data() {
    let cell = loaded(&_static_frame(&holding_a_selector(indexed_frame_cell()), 46, Some(RegId::SS)));
    assert_eq!(RegId::from(cell.addr.unwrap().segment), RegId::SS);
}

/// Every cell of a module frame carried `ss:` where the body never wrote DS:
/// a byte each, 728 over tests/run/qb (pow_unrolled 770 B against main's 684).
#[test]
fn test_a_static_frame_cell_has_no_override_where_the_body_leaves_ds_alone() {
    let cell = loaded(&_static_frame(&loading(indexed_frame_cell()), 46, Some(RegId::SS)));
    assert_eq!(RegId::from(cell.addr.unwrap().segment), RegId::None);
}
