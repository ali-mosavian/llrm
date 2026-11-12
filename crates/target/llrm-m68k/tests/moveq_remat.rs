//! A `moveq` constant is made again where it is read.
//!
//! `recgcd` -O1 held the constant 0 in a saved register across the loop and
//! copied it (`move.l %d7, %d2`), and joined the exits with a `bra`: 4 B more
//! than making the `moveq` again.

use llrm_m68k::compile::{Options, items_bytes, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, lowered_for, object};

fn bytes(
    ir: &str,
    cpu: Cpu,
    level: Level,
    remat_constants: bool,
) -> Result<usize, String> {
    let module = lowered_for(ir, cpu, level).expect("lowers");
    let options = Options { size: level == Level::Os, mid_end: false, remat_constants, ..Options::default() };
    let mut total = 0;
    for (_, global, function) in module.functions() {
        if function.is_declaration() {
            continue;
        }
        let name = global.name.owned().expect("a name");
        total += items_bytes(&items_with(&module, &name, cpu, options)?);
    }
    Ok(total)
}

#[test]
fn recgcd_is_smaller_when_the_zero_is_made_again() {
    let ir = include_str!("data/recgcd.ll");
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let (on, off) = (bytes(ir, cpu, Level::O1, true).unwrap(), bytes(ir, cpu, Level::O1, false).unwrap());
        assert!(on < off, "{cpu:?}: {on} B made again, {off} B held");
    }
}

/// `bench_jpeg_fixed` -O3 68000 failed to compile ("no register for a lane of
/// value N") once constants were remade: the split piece of a `moveq` value
/// stored through a byte view took the order of its `MOVI32ri` result (any
/// register), and greedy put it in `a3`, which has no byte lane.
#[test]
fn a_remade_constant_stored_through_a_byte_view_gets_a_data_register() {
    let ir = include_str!("data/jpeg_fixed_bench.ll");
    object(ir, Cpu::M68000, Level::O3, "jpeg_fixed.c").expect("compiles");
}
