//! The emitter's default shape is the one the private passes were tuned on.
//! `5354c81707` ("(off)") defined a result in its `CopyToReg` register for
//! everyone: 63 corpus cells changed against `release/next` (5 in size, net
//! -8 B, two of them +4 B) with every flag off. Only `Shape::copy_destinations`
//! may do that.

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::mfunc::{MFunction, Shape, emit, emit_with};
use llrm_spike_mf::mfunc::Kind;

fn copies(function: &MFunction) -> usize {
    function.blocks.iter().flat_map(|block| &block.insts).filter(|inst| inst.kind == Kind::Copy).count()
}

#[test]
fn only_the_copy_destination_shape_drops_copies_into_a_copy_register() {
    let mut module = llrm_mir::parse::module(include_str!("fixtures/sieve_pin_o2.ll")).expect("parses");
    llrm_mir::parse::upgrade_lifetimes(&mut module);
    let blocks = llrm_m68k::dagisel::function::select_function(&module, "bench_sieve", Cpu::M68000).expect("selects");
    let shaped =
        emit_with("bench_sieve", &blocks, Shape { copy_destinations: true, ..Shape::default() }).expect("emits");
    let default = emit("bench_sieve", &blocks).expect("emits");
    assert!(
        copies(&shaped) < copies(&default),
        "the shape drops no copy ({} against {}): the test sees nothing",
        copies(&shaped),
        copies(&default)
    );
    let off = emit_with("bench_sieve", &blocks, Shape::default()).expect("emits");
    assert_eq!(format!("{off:?}"), format!("{default:?}"), "the default emit is not the unshaped one");
    // the default keeps the copies the tuned passes read
    assert_eq!(copies(&default), copies(&off));
}
