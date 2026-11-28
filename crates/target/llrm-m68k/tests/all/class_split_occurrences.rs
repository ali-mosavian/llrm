//! The class split walked the whole function once per candidate register to
//! find where it is read and written, so a function of many address values
//! took time in proportion to values x instructions (a very large function:
//! 6% of its compile instructions). It now walks once.

use llrm_m68k::classsplit::{hold_in_data_registers, occurrences_visited};
use llrm_m68k::mfunc::{Block, Class, Inst, Kind, MFunction, Operand, Reg};

/// `values` copies of a data-capable value into an address value.
fn visits(values: u32) -> usize {
    let virt = Reg::Virt;
    let mut function = MFunction::new("f");
    let mut insts = Vec::new();
    function.classes.push(Class::Xr32);
    for k in 0..values {
        let (value, address) = (1 + 2 * k, 2 + 2 * k);
        function.classes.push(Class::Xr32);
        function.classes.push(Class::Ar32);
        insts.push(Inst::new(Kind::Copy, vec![Operand::Def(virt(value)), Operand::Use(virt(0))]));
        insts.push(Inst::new(Kind::Copy, vec![Operand::Def(virt(address)), Operand::Use(virt(value))]));
    }
    function.blocks.push(Block { id: 0, insts, phis: vec![] });
    let before = occurrences_visited();
    hold_in_data_registers(&mut function);
    occurrences_visited() - before
}

#[test]
fn the_class_split_walks_the_function_once_not_once_per_register() {
    let (small, large) = (visits(30), visits(120));
    assert!(small > 0, "premise: the class split visited nothing");
    // 4x the values: 4x the work if linear, 16x if each register walks the
    // function
    assert!(large <= 6 * small, "{small} visits for 30 values, {large} for 120");
}
