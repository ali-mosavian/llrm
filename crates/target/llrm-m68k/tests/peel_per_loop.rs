//! The -Os peel trial peeled every counted loop or none. Each peel copies the
//! body and adds a guard (`cmpi.b #1,Dn; beq`) for the trips left, and some
//! pay for themselves (they fold constants into the copy) and some do not:
//! 32 nested byte loops of known_loads_walks came out at 296 B at the 68000
//! with every peel, 236 B with the paying ones. PEEL-OS-PER-LOOP leaves out
//! each peel the finished code is smaller without.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = include_str!("fixtures/peel_per_loop.ll");

#[test]
fn a_peel_that_does_not_pay_in_bytes_is_left_out() {
    let asm = assembly(IR, cpu_of("68000").unwrap(), Level::Os).expect("compiles");
    let compares = asm.matches("cmpi").count();
    assert!(compares <= 6, "{compares} `cmpi` (every peel kept: 9):\n{asm}");
}
