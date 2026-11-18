//! The -Os peel trial peeled every counted loop it could. Each peel copies the
//! body and adds a guard (`cmpi.b #1,Dn; beq`) for the trips left, and a body
//! of four or more instructions never folded enough to pay: 32 nested byte
//! loops of known_loads_walks came out at 296 B at the 68000 with every peel,
//! 240 B with the small ones (PEEL-OS-THRESHOLD).

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = include_str!("fixtures/peel_per_loop.ll");

#[test]
fn a_large_loop_body_is_not_peeled_at_size() {
    let asm = assembly(IR, cpu_of("68000").unwrap(), Level::Os).expect("compiles");
    let compares = asm.matches("cmpi").count();
    assert!(compares <= 6, "{compares} `cmpi` (every peel kept: 9):\n{asm}");
}
