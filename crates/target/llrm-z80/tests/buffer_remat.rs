//! The remainder of a divide by a constant comes back through a stack buffer
//! the divide routine fills. A use after the next call spilled the loaded
//! remainder into a fresh slot (24 B a site, 15 sites in div_constants_long):
//! the load is made again where it is read now.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

#[test]
fn a_remainder_read_after_a_call_is_loaded_again_not_spilled() {
    let ir = include_str!("kernels/ir/rem_after_call.ll");
    let out = text(ir, "f", 0, &Request::new(Level::Os)).unwrap_or_else(|why| panic!("{why}"));
    let lines: Vec<&str> = out.lines().map(str::trim).collect();
    assert!(
        lines.iter().any(|line| line.starts_with("call __s_udivremsi4")),
        "premise: one divide with a remainder\n{out}"
    );
    let stores = lines.iter().filter(|line| line.starts_with("ld -") && line.contains("(ix),")).count();
    assert_eq!(stores, 4, "only the quotient (4 bytes) is stored\n{out}");
}

/// Every `LD16rf` was a remake candidate once the target named the row, a
/// local variable's load too: `wrapv_overflow_check` and two more grew. Only a
/// load of a buffer a call fills is one.
#[test]
fn a_load_of_a_local_variable_is_not_remade_but_a_load_of_a_call_buffer_is() {
    use llrm_codegen::greedy::tii::TargetInstrInfo;
    use llrm_spike_mf::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
    use llrm_spike_mf::tolir::to_lir;
    use llrm_z80::REGISTER_INFO;
    use llrm_z80::regalloc::{Class, Z80Registers};

    let v = |n: u32| Reg::Virt(n);
    let op = |name, operands| Inst::new(Kind::Op(name), operands);
    let mut f = MFunction::<Class>::new("f");
    f.classes = vec![Class::Gr16; 4];
    let stores = Inst { may_store: true, ..op("STSP16", vec![Operand::Use(v(0)), Operand::Imm(2)]) };
    let call = Inst { call: true, ..op("CALL", vec![Operand::Symbol("__s_udivremsi4".into(), 0)]) };
    let load = |into, frame| Inst {
        may_load: true,
        ..op("LD16rf", vec![Operand::Def(v(into)), Operand::Frame(frame), Operand::Imm(0)])
    };
    let insts = vec![
        op("LEAFI", vec![Operand::Def(v(0)), Operand::Frame(0)]),
        stores,
        call,
        load(1, 0),
        load(2, 1),
        op("USE", vec![Operand::Use(v(1)), Operand::Use(v(2))]),
    ];
    f.blocks.push(Block { id: 0, insts, phis: vec![] });
    let converted = to_lir::<Z80Registers>(&f, llrm_lir::registers::Regs(&REGISTER_INFO)).expect("converts");
    let loads: Vec<_> = converted.body.blocks[0]
        .insns
        .iter()
        .filter(|one| one.what.as_ref().is_some_and(|w| w.name.as_deref() == Some("LD16rf")))
        .collect();
    assert_eq!(loads.len(), 2);
    assert!(converted.tii.remat_shape(loads[0]), "the buffer the call filled");
    assert!(!converted.tii.remat_shape(loads[1]), "a local variable");
}

/// With the address of a global made again where it is read, `copy_runs` spilled `hl` through the class of virtual
/// register 0, a byte: `ld -4(ix), hl`, which the assembler refuses (the program no longer built at -Os).
#[test]
fn a_pinned_pair_is_spilled_as_a_pair_not_a_byte() {
    let ir = include_str!("kernels/ir/copy_runs_os.ll");
    let out = text(ir, "main", 0, &Request::new(Level::Os)).unwrap_or_else(|why| panic!("{why}"));
    assert!(!out.lines().any(|line| line.trim().ends_with("(ix), hl")), "{out}");
}

/// The address of a global was never made again: `is_move_with_reads` refused a move whose source is a symbol, so six
/// loop-invariant addresses were stored once at entry and reloaded (`overflow_builtins` -O2 +56 B, -160 B since).
#[test]
fn the_address_of_a_global_is_made_again_where_it_is_read() {
    use llrm_codegen::greedy::tii::TargetInstrInfo;
    use llrm_spike_mf::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
    use llrm_spike_mf::tolir::to_lir;
    use llrm_z80::REGISTER_INFO;
    use llrm_z80::regalloc::{Class, Z80Registers};

    let op = |name, operands| Inst::new(Kind::Op(name), operands);
    let mut f = MFunction::<Class>::new("f");
    f.classes = vec![Class::Gr16];
    let make = Inst {
        rematerializable: true,
        ..op("LD16ri", vec![Operand::Def(Reg::Virt(0)), Operand::Symbol("_g".into(), 0)])
    };
    let insts = vec![make, op("USE", vec![Operand::Use(Reg::Virt(0))])];
    f.blocks.push(Block { id: 0, insts, phis: vec![] });
    let converted = to_lir::<Z80Registers>(&f, llrm_lir::registers::Regs(&REGISTER_INFO)).expect("converts");
    let made = &converted.body.blocks[0].insns[0];
    assert!(converted.tii.remat_shape(made), "a move of a symbol's address is made again like any literal");
}
