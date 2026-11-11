//! `relax_copy_only_classes`: a data-class value only copies touch is
//! free of the class; one whose byte or word view is read keeps it.
//!
//! The second case failed on `jpeg_fixed` and `jpeg_single` at 68000 -O2 and
//! -O3 (`no register for a lane of value 1690`) when the pass widened every
//! such value: an address register has no byte or word view.

use llrm_m68k::mfunc::{Class, MFunction};
use llrm_m68k::regalloc::relax_copy_only_classes;
use llrm_spike_mf::{Block, Inst, Kind, Operand, Reg};

fn function(
    classes: Vec<Class>,
    insts: Vec<Inst>,
) -> MFunction {
    let mut function = MFunction::new("f");
    function.classes = classes;
    function.blocks.push(Block { id: 0, insts, phis: Vec::new() });
    function
}

#[test]
fn a_value_only_copies_touch_may_take_either_kind() {
    let mut f = function(
        vec![Class::Xr32, Class::Dr32, Class::Xr32],
        vec![Inst::copy(Reg::Virt(1), Reg::Virt(0)), Inst::copy(Reg::Virt(2), Reg::Virt(1))],
    );
    relax_copy_only_classes(&mut f);
    assert_eq!(f.classes[1], Class::Xr32);
}

#[test]
fn a_value_whose_word_view_is_read_stays_a_data_value() {
    let mut f = function(
        vec![Class::Xr32, Class::Dr32, Class::Dr16],
        vec![Inst::copy(Reg::Virt(1), Reg::Virt(0)), Inst::copy(Reg::Virt(2), Reg::Virt(1))],
    );
    relax_copy_only_classes(&mut f);
    assert_eq!(f.classes[1], Class::Dr32, "an address register has no word view");
}

/// `double_across_a_call_result` (68000, all levels) and `jpeg_fixed` and
/// `jpeg_single` at -O1 printed wrong output: the load `if (first)` branches
/// on was widened to `XR32`, took `a1`, and `movea` leaves the codes `beq`
/// reads alone.
#[test]
fn a_value_a_flag_setter_defines_stays_a_data_value() {
    let mut load = Inst::new(Kind::Op("MOV32rp"), vec![Operand::Def(Reg::Virt(1)), Operand::Imm(0), Operand::Frame(0)]);
    load.sets_flags = true;
    let mut f =
        function(vec![Class::Xr32, Class::Dr32, Class::Xr32], vec![load, Inst::copy(Reg::Virt(2), Reg::Virt(1))]);
    relax_copy_only_classes(&mut f);
    assert_eq!(f.classes[1], Class::Dr32, "a move into an address register sets no codes");
}
