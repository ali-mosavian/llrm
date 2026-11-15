//! `share_slots` found no slot-to-slot copy in m68k's reload shape
//! `[Def, Imm(0), Frame]`: the copy's two slots stayed two and a seed-1 style
//! `movem.l (16,%sp),%d0; movem.l %d0,(16,%sp)` pair reached the output.

mod common;

use common::{op, v};
use llrm_spike_mf::spill::share_slots;
use llrm_spike_mf::{Block, MFunction, Operand};

fn store(
    reg: u32,
    slot: i32,
) -> llrm_spike_mf::Inst {
    op("store", vec![Operand::Imm(0), Operand::Frame(slot), Operand::Use(v(reg))])
}

fn reload(
    reg: u32,
    slot: i32,
) -> llrm_spike_mf::Inst {
    op("reload", vec![Operand::Def(v(reg)), Operand::Imm(0), Operand::Frame(slot)])
}

#[test]
fn a_copy_between_slots_in_the_offset_shape_shares_one_slot_and_goes() {
    let mut function = MFunction::new("f");
    function.classes = vec![(); 3];
    function
        .blocks
        .push(
            Block {
                id: 0,
                insts: vec![store(0, 1), reload(1, 1), store(1, 2), reload(2, 2), op("use", vec![Operand::Use(v(2))])],
                phis: vec![],
            },
        );
    let mut slots = vec![(1, ()), (2, ())];
    share_slots(&mut function, &mut slots);
    assert_eq!(slots.len(), 1, "the copy's slots are one");
    let kinds: Vec<_> = function.blocks[0].insts.iter().map(|inst| format!("{:?}", inst.kind)).collect();
    assert_eq!(kinds.len(), 3, "the copy moves nothing: {kinds:?}");
}

/// m68k's `cmpi.b #k, (3,%sp)` read a spilled byte and named no register, so
/// slot sharing took it for no access: two slots live together were joined and
/// the compare read the other value (or, renumbering skipping it, a slot with
/// no store: "an unknown frame index").
#[test]
fn a_read_folded_into_an_instruction_with_no_register_keeps_its_slot_live() {
    let fold = |slot| {
        let mut inst = op("cmpi", vec![Operand::Imm(5), Operand::Imm(3), Operand::Frame(slot)]);
        inst.may_load = true;
        inst
    };
    let mut function = MFunction::new("f");
    function.classes = vec![(); 2];
    function.blocks.push(Block { id: 0, insts: vec![store(0, 1), store(1, 2), fold(1), fold(2)], phis: vec![] });
    let mut slots = vec![(1, ()), (2, ())];
    share_slots(&mut function, &mut slots);
    assert_eq!(slots.len(), 2, "two values live at once share a slot");
}
